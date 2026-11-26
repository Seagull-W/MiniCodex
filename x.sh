#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
cd -- "$(dirname -- "$0")"

case "$#:${1:-}" in
    0:) tests=() ;;
    1:s[1-7]) tests=(--lib --test "${1}_*") ;;
    *) echo "用法: $0 [s1|s2|s3|s4|s5|s6|s7]" >&2; exit 2 ;;
esac

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test "${tests[@]}"
