#!/usr/bin/env bash
set -euo pipefail

VERSION=${1:?"usage: scripts/release-version.sh <version>"}

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?$ ]]; then
  echo "Invalid version format: $VERSION (expected x.y.z or x.y.z-prerelease)" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "Stamping version $VERSION across the repo..."

stamp_json_version() {
  local path=$1

  if [[ -f "$path" ]]; then
    node - "$path" "$VERSION" <<'NODE'
const fs = require('fs');
const path = process.argv[2];
const version = process.argv[3];
const data = JSON.parse(fs.readFileSync(path, 'utf-8'));
data.version = version;
if (data.packages && data.packages['']) {
  data.packages[''].version = version;
}
fs.writeFileSync(path, JSON.stringify(data, null, 2) + '\n');
NODE
    echo "  Stamped $path"
  fi
}

stamp_cargo_package_version() {
  local path=$1

  if [[ -f "$path" ]]; then
    sed -i.bak -E "/^\[package\]/,/^\[/{ s|^version = \".*\"|version = \"$VERSION\"| }" "$path"
    rm -f "$path.bak"
    echo "  Stamped $path"
  fi
}

# 1. Cargo workspace package.version (root Cargo.toml), if present.
if grep -q "^\[workspace.package\]" Cargo.toml; then
  if grep -q "^version = \".*\"" Cargo.toml; then
    sed -i.bak -E "/^\[workspace\.package\]/,/^\[/{ s|^version = \".*\"|version = \"$VERSION\"| }" Cargo.toml
    rm -f Cargo.toml.bak
    echo "  Stamped Cargo.toml workspace.package.version"
  else
    echo "  WARN: Cargo.toml [workspace.package] has no version line; stamping individual crates instead"
  fi
else
  echo "  WARN: Cargo.toml has no [workspace.package] section; stamping individual crates instead"
fi

# 2. Cargo packages with explicit package.version values.
stamp_cargo_package_version crates/plurora-core/Cargo.toml
stamp_cargo_package_version crates/plurora-runtime/Cargo.toml
stamp_cargo_package_version crates/plurora-service/Cargo.toml
stamp_cargo_package_version crates/plurora-cli/Cargo.toml
stamp_cargo_package_version sdk/rust/plurora-contract-sdk/Cargo.toml
stamp_cargo_package_version clients/desktop/src-tauri/Cargo.toml
stamp_cargo_package_version integrations/tdb/rust-adapter/Cargo.toml
stamp_cargo_package_version integrations/tdb/rust-adapter-real-crate/Cargo.toml

# 3. JavaScript package manifests.
stamp_json_version clients/web/package.json
stamp_json_version clients/web/package-lock.json
stamp_json_version clients/desktop/package.json
stamp_json_version clients/desktop/package-lock.json
stamp_json_version sdk/typescript/agentic-forge/package.json
stamp_json_version sdk/typescript/experience-runtime/package.json
stamp_json_version sdk/typescript/experience-runtime/package-lock.json
stamp_json_version sdk/typescript/inference-capability/package.json
stamp_json_version sdk/typescript/contract-sdk/package.json
stamp_json_version sdk/typescript/contract-sdk/package-lock.json
stamp_json_version sdk/typescript/subprocess/package.json
stamp_json_version sdk/typescript/subprocess/package-lock.json

# 4. Tauri app version.
stamp_json_version clients/desktop/src-tauri/tauri.conf.json

# 5. Refresh Rust lockfiles after changing first-party package versions.
cargo metadata --no-deps --format-version 1 >/dev/null
cargo metadata --no-deps --format-version 1 \
  --manifest-path integrations/tdb/rust-adapter/Cargo.toml >/dev/null
cargo metadata --no-deps --format-version 1 \
  --manifest-path integrations/tdb/rust-adapter-real-crate/Cargo.toml >/dev/null

echo "Done. Review changes with 'git diff' and commit."
