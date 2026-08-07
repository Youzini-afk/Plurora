#!/usr/bin/env bash
set -euo pipefail

expected="AGPL-3.0-only"

fail() {
  printf 'license metadata check failed: %s\n' "$1" >&2
  exit 1
}

require_exact_line() {
  local file="$1"
  local line="$2"
  grep -Fqx "$line" "$file" || fail "$file must contain: $line"
}

require_exact_line Cargo.toml 'license = "AGPL-3.0-only"'
require_exact_line clients/desktop/src-tauri/Cargo.toml 'license = "AGPL-3.0-only"'
require_exact_line integrations/tdb/rust-adapter/Cargo.toml 'license = "AGPL-3.0-only"'
require_exact_line integrations/tdb/rust-adapter-real-crate/Cargo.toml 'license = "AGPL-3.0-only"'

for manifest in packages/official/*/manifest.yaml; do
  require_exact_line "$manifest" "license: $expected"
done

node - "$expected" <<'JS'
const fs = require("node:fs");

const expected = process.argv[2];
const packageFiles = [
  "clients/web/package.json",
  "clients/desktop/package.json",
  "sdk/typescript/agentic-forge/package.json",
  "sdk/typescript/experience-runtime/package.json",
  "sdk/typescript/inference-capability/package.json",
  "sdk/typescript/kernel-sdk/package.json",
  "sdk/typescript/subprocess/package.json",
];
const lockFiles = [
  "clients/web/package-lock.json",
  "clients/desktop/package-lock.json",
  "sdk/typescript/experience-runtime/package-lock.json",
  "sdk/typescript/kernel-sdk/package-lock.json",
  "sdk/typescript/subprocess/package-lock.json",
];

const readJson = (path) => JSON.parse(fs.readFileSync(path, "utf8"));
const errors = [];
for (const path of packageFiles) {
  const value = readJson(path).license;
  if (value !== expected) {
    errors.push(`${path}: expected license ${JSON.stringify(expected)}, found ${JSON.stringify(value)}`);
  }
}
for (const path of lockFiles) {
  const value = readJson(path).packages?.[""]?.license;
  if (value !== expected) {
    errors.push(`${path} root package: expected license ${JSON.stringify(expected)}, found ${JSON.stringify(value)}`);
  }
}
if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}
JS

printf 'First-party license metadata is consistently %s.\n' "$expected"
