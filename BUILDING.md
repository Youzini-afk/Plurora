# Building Yggdrasil

Yggdrasil is licensed under AGPL-3.0-only. See [LICENSE](./LICENSE) for terms.

## Source

- Repository: https://github.com/Youzini-afk/Yggdrasil
- Tag a release: `git tag v0.1.0 && git push --tags`

## Prerequisites

### All platforms
- Rust stable (1.78+)
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
- `target/release/ygg` — host CLI

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

```bash
# Terminal 1: host serve
cargo run -p ygg-cli -- host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml

# Terminal 2: web dev server
npm run dev --prefix clients/web

# OR Terminal 2: desktop app
npm run dev --prefix clients/desktop
```

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
