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

PORT_HTTP="${AIRLOCK_HTTP_PORT:-8080}"
PORT_OPCUA_AIRLOCK="${AIRLOCK_OPCUA_PORT:-4841}"
PORT_OPCUA_ECLSS="${ECLSS_OPCUA_PORT:-4842}"
PORT_OPCUA_SABATIER="${SABATIER_OPCUA_PORT:-4843}"
PORT_OPCUA_POWERGRID="${POWERGRID_OPCUA_PORT:-4844}"
NONINTERACTIVE="${AIRLOCK_NONINTERACTIVE:-0}"
AUTO_KILL_PORT="${AIRLOCK_AUTO_KILL_PORT:-0}"
BIND_IP="${AIRLOCK_BIND_IP:-}"
DISPLAY_IP="127.0.0.1"
CMD="${1:-up}"
CONTAINER_NAMES=("mars-airlock-sim")

log_info() { echo -e "  ${CYAN}i${NC} $*"; }
log_ok() { echo -e "  ${GREEN}ok${NC} $*"; }
log_warn() { echo -e "  ${YELLOW}warn${NC} $*"; }
log_err() { echo -e "  ${RED}err${NC} $*"; }
step() { echo -e "\n${BOLD}${CYAN}== $* ==${NC}"; }

is_truthy() {
  case "${1:-0}" in
    1|true|TRUE|yes|YES|on|ON) return 0 ;;
    *) return 1 ;;
  esac
}

require_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    log_err "Required command not found: $cmd"
    exit 1
  fi
}

resolve_compose_cmd() {
  if docker compose version >/dev/null 2>&1; then
    echo "docker compose"
  elif command -v docker-compose >/dev/null 2>&1; then
    echo "docker-compose"
  else
    log_err "docker compose not available (plugin or docker-compose)"
    exit 1
  fi
}

teardown_stack() {
  log_info "Running: $COMPOSE_CMD down --remove-orphans"
  $COMPOSE_CMD down --remove-orphans >/dev/null 2>&1 || true
}

remove_stale_containers() {
  local removed=0
  for cname in "${CONTAINER_NAMES[@]}"; do
    if docker ps -aq --filter "name=^${cname}$" 2>/dev/null | grep -q .; then
      log_warn "Removing stale container: ${cname}"
      docker rm -f "${cname}" >/dev/null 2>&1 || true
      removed=1
    fi
  done

  if [ "$removed" -eq 0 ]; then
    log_ok "No stale containers found"
  fi
}

get_listener_pids() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true
  elif command -v fuser >/dev/null 2>&1; then
    fuser -n tcp "$port" 2>/dev/null || true
  else
    ss -ltnp 2>/dev/null | awk -v p=":${port}" '$4 ~ p {print $NF}' | sed -n 's/.*pid=\([0-9]\+\).*/\1/p' | sort -u
  fi
}

port_has_listener() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -H -ltn "sport = :${port}" 2>/dev/null | grep -q .
    return $?
  fi
  local pids
  pids="$(get_listener_pids "$port" || true)"
  [ -n "$pids" ]
}

describe_pid() {
  local pid="$1"
  ps -p "$pid" -o pid=,user=,comm= 2>/dev/null | sed 's/^ *//'
}

stop_port_listeners() {
  local port="$1"
  local pids
  pids="$(get_listener_pids "$port" | tr ' ' '\n' | sed '/^$/d' | sort -u)"
  [ -z "$pids" ] && return 0

  log_warn "Port ${port} currently in use by:"
  while IFS= read -r pid; do
    [ -z "$pid" ] && continue
    log_info "$(describe_pid "$pid")"
  done <<< "$pids"

  local confirm="N"
  if is_truthy "$NONINTERACTIVE"; then
    if is_truthy "$AUTO_KILL_PORT"; then
      confirm="y"
      log_warn "Non-interactive mode with auto-kill enabled on port ${port}"
    else
      log_err "Non-interactive mode without auto-kill: cannot free port ${port}"
    fi
  else
    echo -ne "  ${BOLD}${YELLOW}Kill listener(s) on port ${port}? [y/N]: ${NC}"
    read -r confirm </dev/tty || true
    confirm="${confirm:-N}"
  fi

  case "$confirm" in
    y|Y|yes|YES)
      while IFS= read -r pid; do
        [ -z "$pid" ] && continue
        if kill -TERM "$pid" 2>/dev/null; then
          log_info "Sent SIGTERM to PID ${pid}"
        fi
      done <<< "$pids"
      sleep 1
      pids="$(get_listener_pids "$port" | tr ' ' '\n' | sed '/^$/d' | sort -u)"
      if [ -n "$pids" ]; then
        while IFS= read -r pid; do
          [ -z "$pid" ] && continue
          if kill -KILL "$pid" 2>/dev/null; then
            log_warn "Sent SIGKILL to PID ${pid}"
          fi
        done <<< "$pids"
        sleep 1
      fi
      ;;
    *)
      log_err "Port ${port} is busy. Aborting startup."
      return 1
      ;;
  esac

  if port_has_listener "$port"; then
    log_err "Port ${port} still busy after cleanup"
    return 1
  fi
  return 0
}

ensure_port_available() {
  local port="$1"
  local pids
  pids="$(get_listener_pids "$port" | tr ' ' '\n' | sed '/^$/d' | sort -u)"

  if [ -z "$pids" ] && ! port_has_listener "$port"; then
    log_ok "Port ${port} available"
    return 0
  fi

  if [ -z "$pids" ] && port_has_listener "$port"; then
    log_err "Port ${port} appears in use, but PID owner is not visible."
    log_info "Try: sudo ss -ltnp 'sport = :${port}'"
    log_info "If this is another compose stack, stop it or set AIRLOCK_OPCUA_PORT/AIRLOCK_HTTP_PORT."
    return 1
  fi

  stop_port_listeners "$port" || return 1
  if port_has_listener "$port"; then
    log_err "Port ${port} still has an active listener."
    log_info "Try: sudo ss -ltnp 'sport = :${port}'"
    return 1
  fi
  log_ok "Port ${port} released"
}

choose_bind_ip() {
  step "Network binding"

  local override="${AIRLOCK_BIND_IP:-}"
  local -a ip_list=()
  local -a ip_labels=()

  ip_list+=("0.0.0.0")
  ip_labels+=("0.0.0.0  - all interfaces (default)")
  ip_list+=("127.0.0.1")
  ip_labels+=("127.0.0.1 - localhost only")

  if command -v ip >/dev/null 2>&1; then
    while IFS= read -r line; do
      local iface
      local ip_addr
      iface="$(echo "$line" | awk '{print $2}')"
      ip_addr="$(echo "$line" | awk '{print $4}' | cut -d/ -f1)"
      case "$iface" in
        lo|docker*|veth*|br-*|virbr*) continue ;;
      esac
      [[ " ${ip_list[*]} " == *" ${ip_addr} "* ]] && continue
      ip_list+=("$ip_addr")
      ip_labels+=("${ip_addr} - ${iface}")
    done < <(ip -4 -o addr show 2>/dev/null)
  fi

  if [ -n "$override" ]; then
    BIND_IP="$override"
    log_info "Using AIRLOCK_BIND_IP=${BIND_IP}"
  elif is_truthy "$NONINTERACTIVE"; then
    BIND_IP="${ip_list[0]}"
    log_info "Non-interactive mode: defaulting bind IP to ${BIND_IP}"
  else
    echo ""
    echo -e "  ${BOLD}Choose bind IP${NC}"
    local i
    for i in "${!ip_labels[@]}"; do
      printf "    ${CYAN}%d)${NC} %s\n" $((i + 1)) "${ip_labels[$i]}"
    done
    echo ""
    while true; do
      echo -ne "  ${BOLD}${YELLOW}Select [1-${#ip_list[@]}] (default 1): ${NC}"
      read -r choice </dev/tty || true
      choice="${choice:-1}"
      if [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le "${#ip_list[@]}" ]; then
        BIND_IP="${ip_list[$((choice - 1))]}"
        break
      fi
      log_err "Invalid selection"
    done
  fi

  if [ "$BIND_IP" = "0.0.0.0" ]; then
    DISPLAY_IP="localhost"
    if [ "${#ip_list[@]}" -ge 3 ]; then
      DISPLAY_IP="${ip_list[2]}"
    fi
  else
    DISPLAY_IP="$BIND_IP"
  fi

  export AIRLOCK_BIND_IP="$BIND_IP"
  export AIRLOCK_OPCUA_PORT="${AIRLOCK_OPCUA_PORT:-$PORT_OPCUA_AIRLOCK}"
  export ECLSS_OPCUA_PORT="${ECLSS_OPCUA_PORT:-$PORT_OPCUA_ECLSS}"
  export SABATIER_OPCUA_PORT="${SABATIER_OPCUA_PORT:-$PORT_OPCUA_SABATIER}"
  export POWERGRID_OPCUA_PORT="${POWERGRID_OPCUA_PORT:-$PORT_OPCUA_POWERGRID}"
  if [ -z "${AIRLOCK_OPCUA_BIND_HOST:-}" ]; then
    export AIRLOCK_OPCUA_BIND_HOST="0.0.0.0"
  fi
  if [ -z "${AIRLOCK_OPCUA_HOST:-}" ]; then
    export AIRLOCK_OPCUA_HOST="$DISPLAY_IP"
  fi
  if [ -z "${AIRLOCK_OPCUA_ENDPOINT_PATH:-}" ]; then
    export AIRLOCK_OPCUA_ENDPOINT_PATH="/underhill/airlock"
  fi
  if [ -z "${AIRLOCK_OPCUA_ENDPOINT_URL:-}" ]; then
    export AIRLOCK_OPCUA_ENDPOINT_URL="opc.tcp://${AIRLOCK_OPCUA_HOST}:${PORT_OPCUA_AIRLOCK}${AIRLOCK_OPCUA_ENDPOINT_PATH}"
  fi

  log_ok "Binding services to ${BIND_IP}"
}

is_ipv4() {
  local candidate="${1:-}"
  [[ "$candidate" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]]
}

ensure_opcua_certificate() {
  step "OPC UA certificate"

  if ! command -v openssl >/dev/null 2>&1; then
    log_warn "openssl not found; using container-generated certificate"
    return 0
  fi

  local pki_dir="./pki"
  local own_dir="${pki_dir}/own"
  local private_dir="${pki_dir}/private"
  local cert_der="${own_dir}/cert.der"
  local cert_pem="${own_dir}/cert.pem"
  local key_pem="${private_dir}/private.pem"
  local host="${AIRLOCK_OPCUA_HOST:-127.0.0.1}"
  local regen_requested="${AIRLOCK_REGEN_CERT:-0}"
  local needs_regen=0

  mkdir -p "$own_dir" "$private_dir"

  if [ ! -s "$cert_der" ] || [ ! -s "$key_pem" ]; then
    needs_regen=1
  fi

  if [ "$needs_regen" -eq 0 ]; then
    if ! openssl x509 -inform DER -in "$cert_der" -noout -text 2>/dev/null | grep -q "URI:urn:mars-airlock:opcua-server"; then
      needs_regen=1
    fi
  fi

  if [ "$needs_regen" -eq 0 ]; then
    if ! openssl x509 -inform DER -in "$cert_der" -noout -text 2>/dev/null | grep -q "$host"; then
      needs_regen=1
    fi
  fi

  if is_truthy "$regen_requested"; then
    needs_regen=1
  fi

  if [ "$needs_regen" -eq 0 ]; then
    log_ok "Using existing OPC UA certificate (${cert_der})"
    return 0
  fi

  local san_entries="DNS:localhost,IP:127.0.0.1,URI:urn:mars-airlock:opcua-server"
  if is_ipv4 "$host"; then
    san_entries="${san_entries},IP:${host}"
  else
    san_entries="${san_entries},DNS:${host}"
  fi

  log_warn "Generating OPC UA certificate with SAN host ${host}"

  openssl genrsa -out "$key_pem" 2048 >/dev/null 2>&1
  openssl req -new -x509 \
    -key "$key_pem" \
    -sha256 \
    -days 365 \
    -subj "/C=US/ST=Mars/O=Underhill/OU=Airlock/CN=Mars Airlock OPC UA Server" \
    -addext "subjectAltName=${san_entries}" \
    -addext "keyUsage=digitalSignature,keyEncipherment,dataEncipherment" \
    -addext "extendedKeyUsage=serverAuth,clientAuth" \
    -out "$cert_pem" >/dev/null 2>&1

  openssl x509 -in "$cert_pem" -outform DER -out "$cert_der" >/dev/null 2>&1
  chmod 600 "$key_pem" || true
  chmod 644 "$cert_der" "$cert_pem" || true

  log_ok "Generated OPC UA certificate (${cert_der})"
}

step "Dependency checks"
require_command docker
COMPOSE_CMD="$(resolve_compose_cmd)"
log_ok "Using compose command: ${COMPOSE_CMD}"

if [ "$CMD" = "down" ]; then
  step "Stopping stack"
  teardown_stack
  remove_stale_containers
  log_ok "Stack stopped"
  exit 0
fi

if [ "$CMD" = "logs" ]; then
  step "Streaming logs"
  $COMPOSE_CMD logs -f
  exit 0
fi

if [ "$CMD" = "build" ]; then
  step "Building image"
  $COMPOSE_CMD build --pull
  log_ok "Build complete"
  exit 0
fi

step "Teardown previous run"
teardown_stack
remove_stale_containers

choose_bind_ip
ensure_opcua_certificate

step "Port checks"
ensure_port_available "$PORT_HTTP"
ensure_port_available "$PORT_OPCUA_AIRLOCK"
ensure_port_available "$PORT_OPCUA_ECLSS"
ensure_port_available "$PORT_OPCUA_SABATIER"
ensure_port_available "$PORT_OPCUA_POWERGRID"

step "Starting stack"
$COMPOSE_CMD up -d --build

step "Status"
$COMPOSE_CMD ps

echo ""
# expose i3X base URL for clients (default aligns with README)
export I3X_HTTP_API="http://${DISPLAY_IP}:${PORT_HTTP}/api/v1"

log_ok "Bind IP: ${AIRLOCK_BIND_IP}"
log_ok "UI: http://${DISPLAY_IP}:${PORT_HTTP}"
log_ok "i3X API: ${I3X_HTTP_API}/namespaces"  # clients can append the namespace path
log_ok "OPC UA Airlock:  opc.tcp://${AIRLOCK_OPCUA_HOST}:${PORT_OPCUA_AIRLOCK}${AIRLOCK_OPCUA_ENDPOINT_PATH:-/underhill/airlock}"
log_ok "OPC UA ECLSS:    opc.tcp://${AIRLOCK_OPCUA_HOST}:${PORT_OPCUA_ECLSS}/underhill/eclss"
log_ok "OPC UA Sabatier: opc.tcp://${AIRLOCK_OPCUA_HOST}:${PORT_OPCUA_SABATIER}/underhill/sabatier"
log_ok "OPC UA PowerGrid: opc.tcp://${AIRLOCK_OPCUA_HOST}:${PORT_OPCUA_POWERGRID}/underhill/powergrid"
log_info "Use ./scripts/launch.sh logs"
log_info "Use ./scripts/launch.sh down"
