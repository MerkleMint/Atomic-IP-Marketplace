#!/usr/bin/env bash
set -euo pipefail

echo "Running all contract tests..."
cargo test --locked --workspace -- --nocapture

echo "All tests passed."
