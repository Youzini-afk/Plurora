#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo run -p plurora-cli --bin export-schemas >/dev/null
cargo run -p plurora-cli --bin validate-schemas
