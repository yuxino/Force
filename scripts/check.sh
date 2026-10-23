#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Match the signed macOS build and CI to reuse the same dependency artifacts.
if [[ "$(uname -s)" == Darwin ]]; then
  export MACOSX_DEPLOYMENT_TARGET=15.0
fi

cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
git diff --check
