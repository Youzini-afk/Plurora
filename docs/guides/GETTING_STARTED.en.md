# Getting started with Plurora

> [English](./GETTING_STARTED.en.md) · [中文](./GETTING_STARTED.md)

This guide is the official distribution's try-it path: start a local Host and Web shell from source, check one example Work, then decide what to read next. It does not turn the official UI into platform law.

## What you need

- Rust 1.78 or newer (`rustc --version`)
- Node.js 20 or newer (`node --version`)
- Git
- A C/C++ toolchain to compile the Host from source: Visual Studio Build Tools 2022 with the C++ desktop workload on Windows

WebView2 and Linux WebKit packages are Desktop-only; see [`../../BUILDING.md`](../../BUILDING.md). If `cl.exe` is missing, `cargo run -p plurora-cli` fails while compiling `ring` / SQLite.

Run every command from the repository root. Until `plurora` is on PATH, write every CLI invocation as `cargo run -p plurora-cli -- <subcommand>`.

## 1. Start the Host

```bash
cargo run -p plurora-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml
```

The first compile takes a few minutes. When it succeeds, the process stays running and listens on `127.0.0.1:8787`. This profile loads first-party Packages through ordinary manifests and does not grant them privilege.

The default data directory is `~/.plurora/` (override with `PLURORA_DATA_DIR`). Do not put those paths in public docs or issues.

## 2. Start the Web shell

In a second terminal:

```bash
npm ci --prefix clients/web
npm run dev --prefix clients/web
```

Open [http://127.0.0.1:1420](http://127.0.0.1:1420). You should see the official Library (Home). Opening an Installation detail does not start a Run.

If the page cannot reach the Host, keep the first terminal running and confirm the browser is on this machine's `127.0.0.1`, not an old process on another port.

## 3. Check the example Work

The Host does not need to be running for this step. `work check` reads local source only; it does not install or start anything:

```bash
cargo run -p plurora-cli -- work check examples/works/modular-simulation --json
```

Success prints Work / Assembly diagnostics rather than an error. The kit is a 5×5 colony simulation with a portable save, a static renderer, and an optional AI Port. It shows that a third party can replace a component on the same public path. See [`MODULAR_SIMULATION.md`](MODULAR_SIMULATION.en.md) for the deeper kit.

The blank play-creation loop, also through public contracts:

```bash
cargo run -p plurora-cli -- play-create-demo
```

## 4. Optional: install the CLI onto PATH

```bash
cargo install --path crates/plurora-cli
plurora work check examples/works/modular-simulation --json
```

`installation` and `realization` subcommands need a running Host. Creating an Installation does not start a Run, and planning a Realization does not apply it.

## Next

| You want | Go to |
|---|---|
| Write a first Package | [`PACKAGE_AUTHORING_WALKTHROUGH.md`](PACKAGE_AUTHORING_WALKTHROUGH.en.md) |
| Understand Installation / Run | [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.en.md) → [`RUN_LIBRARY.md`](RUN_LIBRARY.en.md) |
| Manage API keys | [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md) |
| Build Desktop or a release | [`../../BUILDING.md`](../../BUILDING.md) |
| Understand why the layers exist | [`../CHARTER.md`](../CHARTER.en.md) → [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.en.md) |
| Contribute | [`../../CONTRIBUTING.md`](../../CONTRIBUTING.en.md) |

## If something fails

- **`npm ci` fails:** confirm Node 20+, delete `clients/web/node_modules`, retry.
- **Host compile fails:** confirm Rust 1.78+. On Windows, a missing `cl.exe` means you still need the Visual Studio Build Tools C++ desktop workload.
- **`plurora` is not found:** go back to `cargo run -p plurora-cli -- ...`, or run `cargo install --path crates/plurora-cli` first.
- **You want a live model:** the default profile does not open the network. Copy `profiles/forge-with-live-models.example.yaml` and tighten `allowed_hosts`. See [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md).
