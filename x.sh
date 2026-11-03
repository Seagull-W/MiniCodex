#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
