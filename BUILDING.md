# Building Plurora

Plurora is licensed under AGPL-3.0-only. See [LICENSE](./LICENSE) for terms.

If you only want the Web UI, use the five-minute path below. You still need a C/C++ toolchain to compile the Host from source (Visual Studio Build Tools on Windows). You do not need WebView2 or Linux WebKit packages until you build Desktop. The longer getting-started narrative is in [`docs/guides/GETTING_STARTED.md`](docs/guides/GETTING_STARTED.md).

## Five minutes to the UI

```bash
# Terminal 1
cargo run -p plurora-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml

# Terminal 2
npm ci --prefix clients/web
npm run dev --prefix clients/web
```

Open [http://127.0.0.1:1420](http://127.0.0.1:1420). Until `plurora` is on PATH, invoke every CLI command as `cargo run -p plurora-cli -- <subcommand>`.

## Source

- Repository: https://github.com/Youzini-afk/Plurora
- Tag a release: `git tag v0.1.0 && git push --tags`

## Prerequisites

### All platforms
- Rust stable (1.78+; see `rust-version` in `Cargo.toml`)
- Node.js 20+
- Git

### Linux
- `libwebkit2gtk-4.1-dev`
- `libappindicator3-dev`
- `librsvg2-dev`
- `patchelf`
- `libgtk-3-dev`
- `libsoup-3.0-dev`

Ubuntu 22.04+:
```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev libsoup-3.0-dev
```

### macOS
- Xcode command-line tools (`xcode-select --install`)

### Windows
- Visual Studio Build Tools 2022 with C++ Desktop Development workload
- WebView2 (preinstalled on Windows 11)

## Build

### Rust runtime + CLI
```bash
cargo build --release
```

Outputs:
- `target/release/plurora` — host CLI

### Web client
```bash
npm ci --prefix clients/web
npm run build --prefix clients/web
```

Outputs `clients/web/dist/`.

### Desktop app (Tauri)
```bash
npm ci --prefix clients/desktop
npm run build --prefix clients/desktop
```

Outputs installers in `clients/desktop/src-tauri/target/release/bundle/`:
- Linux: `.deb`, `.rpm`, `.AppImage`
- macOS: `.dmg`, `.app`
- Windows: `.msi`, `.exe`

## Run from source

Web-only development (Host + Vite):

```bash
# Terminal 1
cargo run -p plurora-cli -- host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml

# Terminal 2
npm ci --prefix clients/web
npm run dev --prefix clients/web
```

Desktop development needs the platform extras in Prerequisites, then:

```bash
npm ci --prefix clients/desktop
npm run dev --prefix clients/desktop
```

The Desktop wrapper starts its own loopback Host sidecar. Do not assume port `8787` in that mode.

### If something fails

- Host compile errors: confirm Rust 1.78+ with `rustc --version`.
- `npm ci` fails: confirm Node 20+ with `node --version`, then retry from `clients/web`.
- Browser cannot reach the Host: keep the Host terminal running and confirm it logged a listen address.
- `installation` / `realization` CLI commands fail: start the Host first.
- Desktop only: install the OS extras above (VS Build Tools + WebView2 on Windows).

## Releases

Versions are stamped via `scripts/release-version.sh <version>`.
GitHub Actions workflow `.github/workflows/release.yml` is triggered by `v*` tags
and produces cross-platform installers.

## Verify a release

Each binary release on GitHub corresponds to a tagged commit. Verify:
```bash
git checkout v0.1.0
```

The exact commit and tag are listed in the GitHub release page. Source archives
are also attached to each release.
