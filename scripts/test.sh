#!/usr/bin/env bash
# Run all contract tests with Cargo.lock enforcement and verbose CI output.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "================================================================"
echo "Running contract tests"
echo "  --locked     : Cargo.lock must be up to date"
echo "  --workspace  : All contracts in the workspace"
echo "  --nocapture  : Full output for CI logs"
echo "================================================================"

cargo test \
  --locked \
  --workspace \
  --manifest-path "${ROOT_DIR}/Cargo.toml" \
  -- \
  --nocapture

echo ""
echo "All tests passed."
