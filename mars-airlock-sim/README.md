# Underhill Base Simulator

Standalone Underhill Base simulator with:
- Rust backend (`backend/`)
- Static web frontend (`frontend/`)
- Deterministic simulation loop (`50 ms` default)
- Writable request-variable command model with `Operator` and `Remote` channels
- Staged subsystem writeback model for POL-driven `ECLSS` and `Sabatier` commands
- MTP-aligned runtime model for three PEAs (`Airlock`, `ECLSS`, `Sabatier`)
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

## Runtime Features Implemented

- Command handshake:
  - `Req`: `sequence_id`, `command`, `param1`, `param2`, `execute`
  - `Rsp`: `ack_sequence_id`, `status`, `reject_reason`, `last_update_time_ms`
- Active command telemetry (`source`, `progress_pct`, `blocking_condition`)
- Interlocks and explicit reject reasons
- MTP mode controls (`operation_mode`, `command_en`, `command_en_reason`)
- Permissions toggles (`operator_control_enabled`, `remote_control_enabled`)
- Fault injection (`leak_rate_nominal`)
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
- `POST /api/permissions`
- `POST /api/modes`
- `POST /api/faults/leak-rate`
- `POST /api/commands/operator/write`
- `POST /api/commands/remote/write`
- `GET /ws`

Notes:
- `/api/v1/pea` now returns Airlock + ECLSS + Sabatier PEA descriptors.
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
- Airlock namespace URI: `urn:mars-airlock:mtp`
- ECLSS namespace URI: `urn:underhill:eclss:mtp`
- Sabatier namespace URI: `urn:underhill:sabatier:mtp`

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
