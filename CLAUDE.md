# CLAUDE.md — Project Instructions for Claude Code

## Project Overview

Mars Base Airlock Simulator ("Underhill Base"). Rust backend + Three.js frontend + OPC UA gateway.
Authoritative spec: `functional-specification.MD` in project root.

## Repository Layout

```
Underhill Base/
├── functional-specification.MD   # Full functional spec (source of truth)
├── codex.MD                      # Codex-specific runtime notes (Flatpak sandbox)
├── CLAUDE.md                     # This file
└── mars-airlock-sim/
    ├── backend/                  # Rust (Cargo, edition 2024) — simulation engine + HTTP/WS server
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs           # Axum web server, WS broadcaster, REST API routes
    │       ├── model.rs          # Data model types (Snapshot, commands, enums, MTP structures)
    │       └── sim.rs            # Deterministic simulation loop, command arbitration, interlocks
    ├── opcua-gateway/            # Python (asyncua) — bridges backend REST to OPC UA server
    │   ├── server.py
    │   └── requirements.txt
    ├── frontend/                 # Vanilla JS + Three.js + HTML/CSS
    │   ├── index.html
    │   ├── app.js
    │   └── styles.css
    ├── tests/
    │   ├── opcua/                # Python pytest — backend API + OPC UA gateway tests
    │   │   ├── conftest.py
    │   │   ├── test_backend_api.py
    │   │   ├── test_opcua_gateway.py
    │   │   └── requirements.txt
    │   └── playwright/           # Playwright — web UI integration tests
    │       ├── package.json
    │       ├── playwright.config.js
    │       └── tests/
    │           ├── ui-panels.spec.js
    │           └── commands.spec.js
    ├── scripts/
    │   ├── launch.sh             # Docker compose orchestration script
    │   └── test.sh               # Automated test harness orchestrator
    ├── docker-compose.yml
    ├── docker-compose.test.yml   # Test mode compose (backend + OPC UA gateway + healthcheck)
    ├── Dockerfile
    ├── Dockerfile.opcua-gateway
    └── .env.example
```

## Key Technology Stack

- **Backend**: Rust (edition 2024), Axum 0.8, Tokio, serde/serde_json, tower-http
- **OPC UA Gateway**: Python 3, asyncua, aiohttp (sidecar polling backend REST API)
- **Frontend**: Vanilla HTML/JS/CSS, Three.js (CDN), WebSocket client
- **Infra**: Docker, docker compose

## Running the Project

### Docker compose (preferred)

```bash
cd "/home/earthling/Documents/Underhill Base/mars-airlock-sim"
docker compose up -d --build        # build and start
docker compose logs -f              # follow logs
docker compose down --remove-orphans # stop
```

Or use the launch script:

```bash
cd "/home/earthling/Documents/Underhill Base/mars-airlock-sim"
./scripts/launch.sh up              # build + start (checks ports)
./scripts/launch.sh down            # stop
./scripts/launch.sh logs            # tail logs
./scripts/launch.sh build           # build only
```

Non-interactive mode (CI / agent use):

```bash
AIRLOCK_NONINTERACTIVE=1 AIRLOCK_AUTO_KILL_PORT=1 ./scripts/launch.sh up
```

### Local Cargo build (backend only)

```bash
cd "/home/earthling/Documents/Underhill Base/mars-airlock-sim/backend"
cargo check
cargo build
cargo run                           # serves on http://0.0.0.0:8080
```

### Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `AIRLOCK_HTTP_PORT` | `8080` | Host port for web UI |
| `AIRLOCK_SECURITY_PROFILE` | `NONE` | OPC UA security (`NONE` or `Basic256Sha256`) |
| `AIRLOCK_OPCUA_ENDPOINT_URL` | `opc.tcp://127.0.0.1:4840/mars-airlock` | OPC UA endpoint |
| `RUST_LOG` | `info` | Rust tracing filter |

## Backend Architecture

- **Simulation loop**: 50 ms fixed timestep, deterministic. Publishes WebSocket snapshots at ~10 Hz (every 2nd tick).
- **Command model**: Separate Operator/Remote channels with request/response handshake, rising-edge execute latch, sequence ID idempotency.
- **Interlocks**: Inner/outer doors never open simultaneously; pressure-gated unlock logic.
- **MTP runtime**: Conceptual VDI/VDE/NAMUR 2658 structure exposed via OPC UA browse tree.
- **Pressure model**: `dP/dt = k_eq*f_eq*(P_hab - P) + k_vent*f_vent*(P_mars - P) - k_pump*pump*P - k_leak*(P - P_mars)`

## API Endpoints (backend HTTP)

| Method | Path | Purpose |
|---|---|---|
| GET | `/api/health` | Health check |
| GET | `/api/snapshot` | Current simulation state |
| GET | `/api/events` | Event log |
| GET | `/api/mtp/tree` | MTP browse tree JSON |
| POST | `/api/security/profile` | Set OPC UA security profile |
| POST | `/api/permissions` | Set operator/remote control enable |
| POST | `/api/modes` | Set MTP operation/source modes |
| POST | `/api/faults/leak-rate` | Set leak rate for fault injection |
| POST | `/api/commands/{source}/write` | Write command (source: `operator` or `remote`) |
| GET | `/ws` | WebSocket — receives JSON snapshots at ~10 Hz |

## OPC UA Browse Tree

Full namespace lives under `Objects/MarsBase/AirlockPEA` with sub-nodes: PEAInformationLabel, Diagnostics, ServiceSet (AirlockService → Modes, StateMachine, Procedures, DataAssemblies), Simulation, FaultInjection. See functional-specification.MD section 6 for the complete tree.

## Running Tests

### Full automated test suite (recommended)

```bash
cd "/home/earthling/Documents/Underhill Base/mars-airlock-sim"
./scripts/test.sh
```

This will: build and start the test stack via `docker-compose.test.yml`, run Python backend API tests, run Playwright UI tests, then tear down.

### Test environment variables

| Variable | Default | Purpose |
|---|---|---|
| `TEST_BACKEND_URL` | `http://127.0.0.1:8080` | Backend URL for test clients |
| `TEST_OPCUA_ENDPOINT` | `opc.tcp://127.0.0.1:4840/mars-airlock` | OPC UA endpoint for gateway tests |
| `TEST_OPCUA_ENABLED` | `0` | Set to `1` to run OPC UA gateway browse/write tests |
| `SKIP_BUILD` | `0` | Set to `1` to skip docker build (use existing images) |
| `SKIP_TEARDOWN` | `0` | Set to `1` to leave containers running after tests |
| `RUN_PLAYWRIGHT` | `1` | Set to `0` to skip Playwright tests |
| `RUN_OPCUA_TESTS` | `1` | Set to `0` to skip Python backend API tests |

### Run only backend API tests (no Docker needed if backend already running)

```bash
cd "/home/earthling/Documents/Underhill Base/mars-airlock-sim/tests/opcua"
python3 -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt
TEST_BACKEND_URL=http://127.0.0.1:8080 python -m pytest -v
```

### Run only Playwright tests

```bash
cd "/home/earthling/Documents/Underhill Base/mars-airlock-sim/tests/playwright"
npm install && npx playwright install --with-deps chromium
TEST_BASE_URL=http://127.0.0.1:8080 npx playwright test
```

### Run with OPC UA gateway tests

```bash
TEST_OPCUA_ENABLED=1 ./scripts/test.sh
```

## Development Conventions

- Rust edition 2024; use `cargo fmt` and `cargo clippy` before committing backend changes.
- Scripts use `#!/usr/bin/env bash` with `set -euo pipefail`.
- Git repo is inside `mars-airlock-sim/backend/` (not at project root).
- Keep OPC UA NodeIds and BrowseNames ASCII, space-free, and stable once published.
- Functional spec is the design authority — reference it for any ambiguity.

## Important Notes

- Claude Code runs directly on the host (not sandboxed). No `flatpak-spawn` prefix needed.
- The `codex.MD` file contains equivalent notes for OpenAI Codex which runs in a Flatpak sandbox.
- The backend serves the frontend statically from `../frontend` relative to the Cargo manifest dir.
- The OPC UA gateway is a separate Python sidecar that polls the backend REST API and mirrors state into an OPC UA address space.
