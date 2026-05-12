#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo was not found on PATH." >&2
  exit 1
fi

if command -v npm >/dev/null 2>&1; then
  npm_command=(npm)
elif command -v npm.cmd >/dev/null 2>&1; then
  npm_command=(npm.cmd)
else
  echo "npm was not found on PATH." >&2
  exit 1
fi

service_pids=()

stop_services() {
  for pid in "${service_pids[@]:-}"; do
    if kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
    fi
  done

  wait || true
}

trap stop_services EXIT
trap 'stop_services; exit 130' INT
trap 'stop_services; exit 143' TERM

echo "Starting backend on http://127.0.0.1:9001..."
(
  cd "$script_dir"
  cargo run -- serve
) &
service_pids+=("$!")

echo "Starting frontend on http://127.0.0.1:3000..."
(
  cd "$script_dir/frontend"
  "${npm_command[@]}" run dev
) &
service_pids+=("$!")

if wait -n "${service_pids[@]}"; then
  exit_code=0
else
  exit_code=$?
fi

stop_services
exit "$exit_code"