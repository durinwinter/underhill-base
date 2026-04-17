#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'
BOLD='\033[1m'

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT_DIR"

log_info()  { echo -e "  ${CYAN}i${NC} $*"; }
log_ok()    { echo -e "  ${GREEN}ok${NC} $*"; }
log_warn()  { echo -e "  ${YELLOW}warn${NC} $*"; }
log_err()   { echo -e "  ${RED}err${NC} $*"; }
step()      { echo -e "\n${BOLD}${CYAN}== $* ==${NC}"; }

# Configuration
TEST_BACKEND_URL="${TEST_BACKEND_URL:-http://127.0.0.1:8080}"
TEST_OPCUA_ENDPOINT="${TEST_OPCUA_ENDPOINT:-opc.tcp://127.0.0.1:4841/mars-airlock}"
TEST_OPCUA_ENABLED="${TEST_OPCUA_ENABLED:-0}"
SKIP_BUILD="${SKIP_BUILD:-0}"
SKIP_TEARDOWN="${SKIP_TEARDOWN:-0}"
RUN_PLAYWRIGHT="${RUN_PLAYWRIGHT:-1}"
RUN_OPCUA_TESTS="${RUN_OPCUA_TESTS:-1}"

COMPOSE_FILE="docker-compose.test.yml"
EXIT_CODE=0

resolve_compose_cmd() {
  if docker compose version >/dev/null 2>&1; then
    echo "docker compose"
  elif command -v docker-compose >/dev/null 2>&1; then
    echo "docker-compose"
  else
    log_err "docker compose not available"
    exit 1
  fi
}

cleanup() {
  if [ "$SKIP_TEARDOWN" = "1" ]; then
    log_warn "SKIP_TEARDOWN=1, leaving containers running"
    return
  fi
  step "Tearing down test stack"
  $COMPOSE_CMD -f "$COMPOSE_FILE" down --remove-orphans 2>/dev/null || true
  log_ok "Test stack stopped"
}

wait_for_health() {
  local url="$1"
  local max_wait="${2:-60}"
  local elapsed=0

  while [ "$elapsed" -lt "$max_wait" ]; do
    if curl -sf "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  return 1
}

# ---------------------------------------------------------------------------
step "Dependency checks"
COMPOSE_CMD="$(resolve_compose_cmd)"
log_ok "Using compose command: ${COMPOSE_CMD}"

command -v curl >/dev/null 2>&1 || { log_err "curl required"; exit 1; }

# ---------------------------------------------------------------------------
step "Starting test stack"
trap cleanup EXIT

if [ "$SKIP_BUILD" = "1" ]; then
  $COMPOSE_CMD -f "$COMPOSE_FILE" up -d
else
  $COMPOSE_CMD -f "$COMPOSE_FILE" up -d --build
fi

# ---------------------------------------------------------------------------
step "Waiting for backend health"
if wait_for_health "${TEST_BACKEND_URL}/api/health" 60; then
  log_ok "Backend ready at ${TEST_BACKEND_URL}"
else
  log_err "Backend did not become healthy within 60s"
  $COMPOSE_CMD -f "$COMPOSE_FILE" logs
  exit 1
fi

# ---------------------------------------------------------------------------
# OPC UA tests (Python/pytest)
# ---------------------------------------------------------------------------
if [ "$RUN_OPCUA_TESTS" = "1" ]; then
  step "Running OPC UA / Backend API tests"

  OPCUA_VENV="$ROOT_DIR/tests/opcua/.venv"
  if [ ! -d "$OPCUA_VENV" ]; then
    log_info "Creating Python venv for OPC UA tests"
    python3 -m venv "$OPCUA_VENV"
  fi

  # shellcheck disable=SC1091
  . "$OPCUA_VENV/bin/activate"
  pip install -q -r "$ROOT_DIR/tests/opcua/requirements.txt"

  export TEST_BACKEND_URL
  export TEST_OPCUA_ENDPOINT
  export TEST_OPCUA_ENABLED

  if python -m pytest "$ROOT_DIR/tests/opcua/" -v --tb=short; then
    log_ok "OPC UA / Backend API tests passed"
  else
    log_err "OPC UA / Backend API tests FAILED"
    EXIT_CODE=1
  fi

  deactivate
else
  log_warn "Skipping OPC UA tests (RUN_OPCUA_TESTS=0)"
fi

# ---------------------------------------------------------------------------
# Playwright tests
# ---------------------------------------------------------------------------
if [ "$RUN_PLAYWRIGHT" = "1" ]; then
  step "Running Playwright UI tests"

  PLAYWRIGHT_DIR="$ROOT_DIR/tests/playwright"
  cd "$PLAYWRIGHT_DIR"

  if [ ! -d "node_modules" ]; then
    log_info "Installing Playwright dependencies"
    npm install
    npx playwright install --with-deps chromium
  fi

  export TEST_BASE_URL="$TEST_BACKEND_URL"

  if npx playwright test; then
    log_ok "Playwright UI tests passed"
  else
    log_err "Playwright UI tests FAILED"
    EXIT_CODE=1
  fi

  cd "$ROOT_DIR"
else
  log_warn "Skipping Playwright tests (RUN_PLAYWRIGHT=0)"
fi

# ---------------------------------------------------------------------------
step "Test Summary"
if [ "$EXIT_CODE" -eq 0 ]; then
  log_ok "All tests passed"
else
  log_err "Some tests failed (exit code $EXIT_CODE)"
fi

exit "$EXIT_CODE"
