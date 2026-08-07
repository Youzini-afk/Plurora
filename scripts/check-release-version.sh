#!/usr/bin/env bash
set -euo pipefail

INPUT=${1:?'usage: scripts/check-release-version.sh <vX.Y.Z>'}
VERSION=${INPUT#v}

if [[ "$INPUT" != v* ]] || [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?$ ]]; then
  echo "Invalid release tag: $INPUT (expected vX.Y.Z or vX.Y.Z-prerelease)" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo_manifests=(
  crates/plurora-core/Cargo.toml
  crates/plurora-runtime/Cargo.toml
  crates/plurora-service/Cargo.toml
  crates/plurora-cli/Cargo.toml
  sdk/rust/plurora-contract-sdk/Cargo.toml
  clients/desktop/src-tauri/Cargo.toml
  integrations/tdb/rust-adapter/Cargo.toml
  integrations/tdb/rust-adapter-real-crate/Cargo.toml
)

for manifest in "${cargo_manifests[@]}"; do
  actual=$(cargo metadata --no-deps --format-version 1 --manifest-path "$manifest" | node -e '
    const path = require("path");
    const wanted = path.resolve(process.argv[1]);
    let raw = "";
    process.stdin.on("data", chunk => raw += chunk);
    process.stdin.on("end", () => {
      const package = JSON.parse(raw).packages.find(
        candidate => path.resolve(candidate.manifest_path) === wanted,
      );
      if (!package) throw new Error(`cargo metadata omitted ${wanted}`);
      process.stdout.write(package.version);
    });
  ' "$manifest")
  if [[ "$actual" != "$VERSION" ]]; then
    echo "$manifest has version $actual; expected $VERSION" >&2
    exit 1
  fi
done

node - "$VERSION" <<'NODE'
const fs = require('fs');

const expected = process.argv[2];
const files = [
  'clients/web/package.json',
  'clients/web/package-lock.json',
  'clients/desktop/package.json',
  'clients/desktop/package-lock.json',
  'clients/desktop/src-tauri/tauri.conf.json',
  'sdk/typescript/agentic-forge/package.json',
  'sdk/typescript/experience-runtime/package.json',
  'sdk/typescript/experience-runtime/package-lock.json',
  'sdk/typescript/inference-capability/package.json',
  'sdk/typescript/contract-sdk/package.json',
  'sdk/typescript/contract-sdk/package-lock.json',
  'sdk/typescript/subprocess/package.json',
  'sdk/typescript/subprocess/package-lock.json',
];

for (const path of files) {
  const data = JSON.parse(fs.readFileSync(path, 'utf8'));
  if (data.version !== expected) {
    throw new Error(`${path} has version ${data.version}; expected ${expected}`);
  }
  if (data.packages?.[''] && data.packages[''].version !== expected) {
    throw new Error(`${path} root lock package has version ${data.packages[''].version}; expected ${expected}`);
  }
}
NODE

if [[ -n "$(git status --porcelain -- Cargo.lock integrations/tdb/*/Cargo.lock)" ]]; then
  echo "Release version validation changed a Rust lockfile; run scripts/release-version.sh and commit it" >&2
  git status --short -- Cargo.lock integrations/tdb/*/Cargo.lock >&2
  exit 1
fi

echo "Release tag $INPUT matches all first-party package versions."
