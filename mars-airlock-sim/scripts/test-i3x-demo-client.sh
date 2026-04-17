#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
KEEP_UP="${KEEP_UP:-0}"

if command -v flatpak-spawn >/dev/null 2>&1; then
  RUN_HOST=(flatpak-spawn --host /usr/bin/env bash -lc)
else
  RUN_HOST=(/usr/bin/env bash -lc)
fi

run_host() {
  "${RUN_HOST[@]}" "$1"
}

cleanup() {
  if [ "$KEEP_UP" = "1" ]; then
    return
  fi
  run_host "cd \"$ROOT_DIR\" && docker compose down --remove-orphans >/dev/null 2>&1 || true"
}
trap cleanup EXIT

echo "Starting Underhill server stack..."
run_host "cd \"$ROOT_DIR\" && docker compose up -d --build"

echo "Running i3X demo-client tests..."
run_host "
  set -euo pipefail
  cd \"$ROOT_DIR/tests/opcua\"
  if [ ! -d .venv ]; then
    python3 -m venv .venv
  fi
  . .venv/bin/activate
  pip install -q -r requirements.txt
  TEST_BACKEND_URL=http://127.0.0.1:8080 pytest -q test_i3x_demo_client.py
"

echo "i3X demo-client tests passed."
