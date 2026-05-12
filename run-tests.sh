#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if command -v npm.cmd >/dev/null 2>&1; then
  npm_command=(npm.cmd)
else
  npm_command=(npm)
fi

echo "Running Rust tests..."
cargo test --manifest-path "$script_dir/Cargo.toml"

echo
echo "Running frontend tests..."
(cd "$script_dir/frontend" && "${npm_command[@]}" test)
