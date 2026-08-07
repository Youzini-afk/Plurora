#!/bin/bash
set -euo pipefail

cargo run -p plurora-cli --bin export-schemas
cargo run -p plurora-cli --bin generate-sdks
echo "SDKs regenerated. Review and commit changes under sdk/."
