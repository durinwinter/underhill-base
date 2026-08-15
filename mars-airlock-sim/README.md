# Underhill Base Simulator

Standalone Underhill Base simulator with:
- Rust backend (`backend/`)
- Static web frontend (`frontend/`)
- Deterministic simulation loop (`50 ms` default)
- Continuous plant clock with live or accelerated fixed-step operation
- Deterministic fast/medium/slow scheduler boundaries for continuous subsystem evolution
- Writable request-variable command model with `Operator` and `Remote` channels
- Staged subsystem writeback model for POL-driven `ECLSS` and `Sabatier` commands
- MTP-aligned runtime model for four PEAs (`Airlock`, `ECLSS`, `Sabatier`, `Power`)
- Native OPC UA servers via Rust crate `async-opcua` (one endpoint per PEA)
- UNS publishing over Zenoh and/or MQTT
- i3X-compatible HTTP API (`/api/v1/*`)
- Real-time WebSocket snapshots for UI
- Fendt-themed HMI + P&ID and live event log
- Tabbed local operator HMIs for `Airlock`, `ECLSS`, and `Sabatier`
- PEA set:
  - `AIRLOCK-PEA-001` (airlock state-machine and command model)
  - `ECLSS-PEA-001` (life-support dynamics: CO2 scrub, O2 generation, humidity/water recovery)
  - `SABATIER-PEA-001` (CO2 methanation dynamics: CH4/H2O production, reactor telemetry)
  - `POWER-PEA-001` (islanded DC microgrid: solar/fission generation, battery, bus, load shedding)

## Runtime Features Implemented

- Command handshake:
  - `Req`: `sequence_id`, `command`, `param1`, `param2`, `execute`
  - `Rsp`: `ack_sequence_id`, `status`, `reject_reason`, `last_update_time_ms`
- Active command telemetry (`source`, `progress_pct`, `blocking_condition`)
- Interlocks and explicit reject reasons
- MTP mode controls (`operation_mode`, `command_en`, `command_en_reason`)
- Permissions toggles (`operator_control_enabled`, `remote_control_enabled`)
- Fault injection (`leak_rate_nominal`, valve stiction/bias/rate/stuck/leakage)
- Diagnostics and connected session reporting
- ECLSS/Sabatier writeback lifecycle: `PENDING`, `APPLIED`, `SETTLING`, `COMPLETE`, `REJECTED`, `TIMED_OUT`

## Run Locally (No Containers)

From `mars-airlock-sim/backend`:

```bash
cargo run
```

App URL:
- `http://127.0.0.1:8080`

Environment:

```bash
AIRLOCK_SECURITY_PROFILE=NONE cargo run
```

Allowed profile values:
- `NONE`
- `BASIC256SHA256`

Optional OPC UA env:
- `AIRLOCK_OPCUA_BIND_HOST` (default `0.0.0.0`, socket bind address)
- `AIRLOCK_OPCUA_HOST` (default `127.0.0.1`, advertised endpoint host)
- `AIRLOCK_OPCUA_PORT` (optional explicit port override for Airlock PEA)
- `ECLSS_OPCUA_PORT` (optional explicit port override for ECLSS PEA)
- `SABATIER_OPCUA_PORT` (optional explicit port override for Sabatier PEA)
- `POWERGRID_OPCUA_PORT` (optional explicit port override for Power PEA)
- `AIRLOCK_OPCUA_ENDPOINT_PATH` (default `/underhill/airlock`)
- `UNDERHILL_OPCUA_PORT_RANGE` (default `4841-4899`)
- `UNDERHILL_OPCUA_PORT_ALLOCATIONS_FILE` (default `backend/data/opcua_port_allocations.json`)
- `AIRLOCK_REGEN_CERT=1` (launcher: force regeneration of OPC UA cert in `./pki`)

OPC UA ports are now reserved per PEA and persisted in the allocation file so known `pea_id`s keep stable endpoint ports across restarts.

Optional UNS/Zenoh env:
- `ZENOH_ROUTER` (example: `tcp/127.0.0.1:7447`)
- `MURPH_NODE_ID` (default: `local`)
- `UNS_MQTT_BROKER` (example: `mqtt://127.0.0.1:1883`)
- `UNS_MQTT_CLIENT_ID` (default: `underhill-uns-publisher`)
- `UNS_MQTT_USERNAME` (optional)
- `UNS_MQTT_PASSWORD` (optional)

Continuous runtime env:
- `UNDERHILL_TIME_SCALE` (default `1.0`; values above `1.0` accelerate plant time)
- `UNDERHILL_WALL_TICK_MS` (default `50`; host scheduler wake-up period)
- `UNDERHILL_MEDIUM_PERIOD_SEC` (default `1.0`)
- `UNDERHILL_SLOW_PERIOD_SEC` (default `60.0`)
- `UNDERHILL_MAX_STEPS_PER_WALL_TICK` (default `2000`; excess work remains visible as backlog)
- `UNDERHILL_PLANT_ID` (default `underhill-base-primary`; stable identity checked on restore)
- `UNDERHILL_STATE_DIR` (default `backend/data/continuous`)
- `UNDERHILL_CHECKPOINT_INTERVAL_SEC` (default `60` simulated seconds)
- `UNDERHILL_DOWNTIME_POLICY` (`catch_up` by default; `freeze` is intended for validation forks)
- `UNDERHILL_MAX_DOWNTIME_CATCHUP_SEC` (default `604800`; larger gaps require operator recovery)

The physics integrator always advances in deterministic 50 ms steps. Acceleration executes more
fixed steps per host tick rather than increasing the physical step size. `GET /api/health` exposes
plant elapsed time, Mars sol, time scale, step index, catch-up backlog, and persistence settings.
The backend atomically checkpoints the complete Airlock, ECLSS, Sabatier, Power, PEA runtime, operator,
and scheduler state, retains the preceding checkpoint as a recovery fallback, and restores the
same plant identity on restart. Lifecycle records are appended to `plant-events.ndjson`. Docker
Compose mounts `backend/data`, so container replacement does not discard the represented plant.
On canonical restart, elapsed wall time since the checkpoint is queued as deterministic fixed-step
catch-up work. Catch-up is bounded and visible through `/api/health`; an excessive gap stops startup
instead of silently inventing history. Setting the policy to `freeze` is an explicit fork behavior.
SIGINT and SIGTERM coordinate HTTP shutdown with the plant scheduler and force a synced final
checkpoint plus a `shutdown_checkpoint_saved` journal record before the backend exits.
Physics steps, HTTP state mutations, OPC UA commands, operational journal appends, and checkpoint
capture share a plant transaction gate. Each checkpoint records the exact journal sequence it
covers, allowing recovery code to identify—not silently ignore—the durable post-checkpoint tail.

Airlock valve commands no longer change physical position instantaneously. The equalization and
vent valves model rate-limited lag, deadband, stiction, hard-stuck faults, leakage, and sensor bias.
HTTP snapshots and OPC UA expose commanded, true, and sensed position, command/sensor residual,
and stiction state so controllers and diagnostic agents can be evaluated against non-ideal loops.

The Power PEA continuously couples actual Airlock, ECLSS, and Sabatier demand to deterministic
Mars-sol solar input, steady fission generation, battery charge/discharge limits and efficiencies,
DC-bus voltage, flexible-load shedding, unmet critical load, and cumulative energy counters. Its
instantaneous balance residual is exposed so agents can verify conservation rather than trusting
plausible-looking independent signals. Power state and energy integrals survive canonical restarts.

The versioned canonical telemetry catalog defines the fully formed base envelope before every
physical subsystem is implemented. `GET /api/v1/telemetry/stats` reports 110,000 stable interface
tags across 13 subsystem families; `GET /api/v1/telemetry/catalog` provides filtered pagination by
`subsystem_family`, `owner_pea`, `publication_class`, and `activation_state`. Each definition carries its owner,
equipment path, semantic role, value type, engineering unit, range, internal/sensing/publication
cadences, deadband, criticality, quality states, retention class, and OPC UA subscription class.
The catalog is a discoverable contract and capacity budget—not a claim that every catalog tag is
already backed by implemented physics. Physics-backed activation is tracked separately as PEAs
graduate from planned definitions to sensed values.

When Zenoh and/or MQTT are configured, the backend publishes:
- `murph/habitat/nodes/{node_id}/pea/{pea_id}/announce`
- `murph/habitat/nodes/{node_id}/pea/{pea_id}/status`
- `murph/habitat/nodes/{node_id}/pea/{pea_id}/services/{service_tag}/state`
- `murph/habitat/nodes/{node_id}/pea/{pea_id}/data/{tag}`

## Containerized Run (Docker Compose)

From `mars-airlock-sim`:

```bash
docker compose up -d --build
```

Or use launcher:

```bash
./scripts/launch.sh up
./scripts/launch.sh logs
./scripts/launch.sh down
```

The launcher generates/persists OPC UA certs in `./pki` and includes the selected OPC UA host in SAN.

Host URL:
- `http://127.0.0.1:${AIRLOCK_HTTP_PORT:-8080}`
- `opc.tcp://127.0.0.1:${AIRLOCK_OPCUA_PORT:-4841}${AIRLOCK_OPCUA_ENDPOINT_PATH:-/underhill/airlock}`
- `opc.tcp://127.0.0.1:${ECLSS_OPCUA_PORT:-4842}/underhill/eclss`
- `opc.tcp://127.0.0.1:${SABATIER_OPCUA_PORT:-4843}/underhill/sabatier`
- `opc.tcp://127.0.0.1:${POWERGRID_OPCUA_PORT:-4844}/underhill/power`
- i3X endpoints start at: `http://127.0.0.1:${AIRLOCK_HTTP_PORT:-8080}/api/v1/namespaces`

Optional env file:

```bash
cp .env.example .env
```

If running from a sandboxed shell, use host execution:

```bash
flatpak-spawn --host /usr/bin/env bash -lc 'cd "/home/earthling/Documents/Focus/Underhill Base/mars-airlock-sim" && ./scripts/launch.sh up'
```

## Client Connectivity

- UAExpert:
  - `opc.tcp://127.0.0.1:4841/underhill/airlock`
  - `opc.tcp://127.0.0.1:4842/underhill/eclss`
  - `opc.tcp://127.0.0.1:4843/underhill/sabatier`
  - `opc.tcp://127.0.0.1:4844/underhill/power`
- MQTT Explorer:
  - Set `UNS_MQTT_BROKER` (for example `mqtt://127.0.0.1:1883`)
  - Browse from topic root `murph/habitat/nodes/{node_id}/pea/`
- i3X Explorer:
  - Base URL: `http://127.0.0.1:8080/api/v1`
  - Entry endpoints: `/namespaces`, `/objecttypes`, `/objects`

## i3X Demo-Client Test

Run the automated i3X checks that import the shared Focus demo client (`/home/earthling/Documents/Focus/test_client.py`) and validate Underhill `/api/v1` responses:

```bash
./scripts/test-i3x-demo-client.sh
```

Set `KEEP_UP=1` to leave Docker running after the test:

```bash
KEEP_UP=1 ./scripts/test-i3x-demo-client.sh
```

From a sandboxed shell, run:

```bash
flatpak-spawn --host /usr/bin/env bash -lc 'cd "/home/earthling/Documents/Focus/Underhill Base/mars-airlock-sim" && ./scripts/test-i3x-demo-client.sh'
```

The script auto-detects whether `flatpak-spawn` is available; on a normal host shell it runs directly.

## API Surface

- `GET /api/health`
- `GET /api/snapshot`
- `GET /api/v1/power/snapshot`
- `GET /api/v1/telemetry/stats`
- `GET /api/v1/telemetry/catalog?subsystem_family=power&publication_class=fast&offset=0&limit=250`
- `GET /api/events`
- `GET /api/mtp/tree`
- `GET /api/v1/pea`
- `GET /api/v1/pea/{pea_id}`
- `POST /api/v1/pea/{pea_id}/deploy`
- `POST /api/v1/pea/{pea_id}/start`
- `POST /api/v1/pea/{pea_id}/stop`
- `POST /api/v1/pea/{pea_id}/undeploy`
- `GET /api/v1/pea/{pea_id}/opcua`
- `GET /api/v1/pea/{pea_id}/mtp/tree`
- `GET /api/v1/pea/{pea_id}/operator-state` (`ECLSS-PEA-001`, `SABATIER-PEA-001`)
- `POST /api/v1/pea/{pea_id}/operator-state` (`ECLSS-PEA-001`, `SABATIER-PEA-001`)
- `POST /api/v1/pea/{pea_id}/services/{service_tag}/command`
- `GET /api/v2/compatibility/mtp-pascalcase-map`
- `GET /api/v1/i3x/pea`
- `GET /api/v1/i3x/pea/{pea_id}`
- `GET /api/v1/i3x/capability-schema`
- `GET /api/v1/namespaces`
- `GET /api/v1/objecttypes`
- `GET /api/v1/objecttypes/{element_id}`
- `GET /api/v1/relationshiptypes`
- `GET /api/v1/relationshiptypes/{element_id}`
- `GET /api/v1/objects`
- `GET /api/v1/objects/{element_id}`
- `GET /api/v1/objects/{element_id}/related`
- `GET /api/v1/objects/{element_id}/value`
- `PUT /api/v1/objects/{element_id}/value` (stubbed, returns `501`)
- `GET /api/v1/objects/{element_id}/history`
- `POST /api/security/profile`
- `POST /api/faults/valve` (`equalize` or `vent`; configures actuator fault parameters)
- `POST /api/permissions`
- `POST /api/modes`
- `POST /api/faults/leak-rate`
- `POST /api/commands/operator/write`
- `POST /api/commands/remote/write`
- `GET /ws`

Notes:
- `/api/v1/pea` now returns Airlock + ECLSS + Sabatier + Power PEA descriptors.
- ECLSS/Sabatier currently support lifecycle simulation + staged writeback + UNS publication; service command endpoint remains Airlock-only for now.
- WinCC OA/POL integration notes: `WINCCOA_POL_INTEGRATION.md`
- AI agent base brief: `../MARS_BASE_AGENT_BRIEF.md`
- Airlock command endpoint accepts legacy `AirlockService` plus canonical PascalCase service tags: `Depressurizing`, `Pressurizing`, `HatchTransfer`, `AtmosphereReclaim`, `EmergencyRepressurizing`, `Isolation`.
- V2 service definitions include control module metadata with constrained types: `BinVlv`, `AnaVlv`, `BinDrv`, `AnaDrv`.

PEA package artifacts:
- Canonical mapping YAML template: `backend/spec/pea-canonical-mapping.yaml`
- Generated browse-tree manifest example: `backend/spec/generated/underhill-base-browse-tree.manifest.yaml`
- Rust per-PEA endpoint host skeleton: `backend/src/pea_endpoint_host.rs`

## OPC UA Surface

- Airlock PEA endpoint: `opc.tcp://127.0.0.1:4841/underhill/airlock`
- ECLSS PEA endpoint: `opc.tcp://127.0.0.1:4842/underhill/eclss`
- Sabatier PEA endpoint: `opc.tcp://127.0.0.1:4843/underhill/sabatier`
- Power PEA endpoint: `opc.tcp://127.0.0.1:4844/underhill/power`
- Airlock namespace URI: `urn:mars-airlock:mtp`
- ECLSS namespace URI: `urn:underhill:eclss:mtp`
- Sabatier namespace URI: `urn:underhill:sabatier:mtp`
- Power namespace URI: `urn:underhill:power:mtp`

## Command Write Payload Example

```json
{
  "sequence_id": 42,
  "command": "START_DEPRESSURIZE_CYCLE",
  "param1": 0.0,
  "param2": 0.0,
  "execute": true
}
```

Expected command pulse:
1. write with `execute=true`
2. write same request with `execute=false`
