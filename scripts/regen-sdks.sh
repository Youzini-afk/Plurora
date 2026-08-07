#!/bin/bash
set -euo pipefail

cargo run -p plurora-cli --bin export-schemas
cargo run -p plurora-cli --bin generate-sdks
cargo run -p plurora-cli --bin validate-schemas
cargo fmt --all
echo "Schemas and SDKs regenerated, validated, and formatted. Review and commit changes under docs/spec/v1/schemas and sdk/."
