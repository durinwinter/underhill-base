# Functional Specification v1.1
## Standalone Underhill Base PEA Simulator Platform (Rust + Three.js + OPC UA + MTP Data Model)

## 1. Document Status
- Status: Draft for implementation.
- Date: February 26, 2026.
- Owner: Project team.
- Decision baseline:
1. Standalone application.
2. OPC UA command interface uses writable request variables only.
3. MTP structure is mandatory from v1 as a data model.
4. Fendt-themed HMI is a hard requirement.
5. Security mode is user-selectable, with `None` allowed and defaultable.
6. Operator and Remote command channels are separate.
7. Priority order: OPC UA interoperability first, then simulation and UI completeness.

## 2. Purpose
Build a deterministic subsystem simulator platform for Mars habitat operations where each subsystem is a PEA with its own MTP-conformant OPC UA endpoint, and render base operations in a real-time web interface with three required panels:
1. OPC UA Connectivity and Diagnostics.
2. 3D Real-Time Airlock View.
3. Fendt-themed HMI Dashboard with P&ID.

The simulator shall be suitable for PLC/SCADA/UAExpert integration testing and for POL orchestration across multiple PEAs.

## 3. Scope
### 3.1 In Scope
1. Standalone Rust backend simulation engine.
2. Built-in OPC UA server with browseable namespace.
3. Writable request-variable command model with deterministic arbitration.
4. Mandatory MTP-style data model objects in OPC UA runtime structure.
5. Web frontend with Three.js and Fendt-themed operational dashboard.
6. Live data feed from backend to frontend by WebSocket.
7. Command source separation for Operator UI vs Remote OPC UA clients.
8. Multi-PEA subsystem runtime (Airlock, ECLSS, Sabatier, and future subsystems).
9. One OPC UA endpoint per subsystem PEA with stable port reservation.
10. Canonical mapping definitions that compile to Rust runtime mapping and optional PLC ST outputs.

### 3.2 Out of Scope (v1)
1. Full standards-certified MTP package generation and export.
2. Direct browser OPC UA transport; browser uses backend WebSocket.
3. Full autonomous base-level coordination without POL sequencing.
4. Advanced CFD-level gas dynamics.

## 4. System Architecture
### 4.1 Components
1. Backend (Rust):
- Deterministic simulation loop.
- OPC UA server.
- Command validator and arbitration logic.
- MTP runtime object model publisher.
- WebSocket broadcaster.

2. Frontend (Web):
- Single-page app with 3 required interfaces.
- Three.js renderer for physical scene.
- Fendt-themed HMI and P&ID.
- OPC UA diagnostics panel.

### 4.2 Authoritative Data Flow
1. Command path:
- PLC/UAExpert/UI writes request variables in OPC UA.
- Backend validates state, interlocks, mode, permissions.
- Backend accepts or rejects with explicit reason.
- Backend updates active command telemetry and simulation state.
- Backend publishes OPC UA telemetry and WebSocket snapshots.

2. Visualization path:
- UI renders only backend snapshots.
- UI does not invent derived state for control logic.

## 5. Core Functional Requirements
## 5.1 Simulation Engine
1. Fixed timestep loop, default `50 ms`, configurable.
2. Deterministic outcomes for identical initial state and command sequence.
3. State includes at minimum:
- Chamber pressure, temperature, and gas composition.
- Door positions and lock states.
- Valve positions and pump status.
- Alarm flags and fault injection status.
- Current procedural/operational state.

4. Pressure dynamics shall respond to:
- Equalization valve coupling to habitat pressure.
- Vent valve coupling to Mars exterior pressure.
- Pump evacuation effect.
- Leak term.

5. A stable reference model shall be implemented (exact coefficients configurable):
- `dP/dt = k_eq * f_eq * (P_hab - P_chamber) + k_vent * f_vent * (P_mars - P_chamber) - k_pump * pump_on * P_chamber - k_leak * (P_chamber - P_mars)`

6. Interlocks:
- Inner and outer doors shall never be open simultaneously.
- Outer unlock blocked above `OuterUnlockMaxPressurePa`.
- Inner unlock blocked below `InnerUnlockMinPressurePa`.

## 5.2 OPC UA Server
1. Endpoint base: `opc.tcp://<host>:4840/mars-airlock`.
2. Security profile is user-selectable at startup:
- `None` profile.
- `Basic256Sha256` profile.
3. Default profile may be `None` for local testing.
4. Namespace shall be stable and browse-friendly.
5. OPC UA methods are not required in v1.
6. All external commands shall be writable request variables.

## 5.3 Command Model (Writable Requests Only)
### 5.3.1 Separate Channels
1. `Commands/Operator/*` for UI-originated writes.
2. `Commands/Remote/*` for PLC/UAExpert/SCADA writes.

### 5.3.2 Request/Response Handshake
Each channel shall expose:
1. Request (`Req`) writable fields:
- `SequenceId` (UInt32)
- `Command` (UInt16 enum)
- `Param1` (Double)
- `Param2` (Double)
- `Execute` (Boolean pulse)

2. Response (`Rsp`) read-only fields:
- `AckSequenceId` (UInt32)
- `Status` (UInt16 enum)
- `RejectReason` (String)
- `LastUpdateTime` (DateTime)

3. Global active telemetry (`ActiveCommand`) read-only fields:
- `Command`
- `Source` (`OPERATOR_UI`, `REMOTE_OPCUA`, `SYSTEM_AUTO`)
- `SequenceId`
- `Param1`
- `Param2`
- `State`
- `ProgressPct`
- `BlockingCondition`
- `StartTime`
- `LastUpdateTime`

### 5.3.3 Arbitration and Idempotency
1. Command latches on `Execute` rising edge.
2. Duplicate `SequenceId` from same source shall be ignored idempotently.
3. One active command at a time, except `ABORT_CYCLE` can be allowed during active execution.
4. If busy, reject with explicit reason.

### 5.3.4 Permissions
1. `Permissions/OperatorControlEnabled`.
2. `Permissions/RemoteControlEnabled`.
3. Source-disabled requests shall be rejected with explicit reason.

### 5.3.5 Required Command Enum
`CommandEnum`:
1. `NONE`
2. `START_DEPRESSURIZE_CYCLE`
3. `START_PRESSURIZE_CYCLE`
4. `ABORT_CYCLE`
5. `RESET_FAULTS`
6. `SET_PUMP_ON`
7. `SET_EQUALIZE_VALVE_PCT`
8. `SET_VENT_VALVE_PCT`
9. `SET_INNER_DOOR_TARGET_PCT`
10. `SET_OUTER_DOOR_TARGET_PCT`
11. `LOCK_INNER_DOOR`
12. `UNLOCK_INNER_DOOR`
13. `LOCK_OUTER_DOOR`
14. `UNLOCK_OUTER_DOOR`

### 5.3.6 Required Status Enum
`CommandStatusEnum`:
1. `IDLE`
2. `ACCEPTED`
3. `REJECTED`
4. `RUNNING`
5. `COMPLETE`
6. `ABORTED`

### 5.3.7 Reject Reason Quality
Reject reasons shall be human-readable and explicit, including at minimum:
1. `Interlock: inner door open`.
2. `Interlock: outer door open`.
3. `Blocked: pressure too high for outer unlock`.
4. `Blocked: pressure too low for inner unlock`.
5. `Busy: command already active`.
6. `Permissions: operator control disabled`.
7. `Permissions: remote control disabled`.
8. `Faulted: reset required`.

## 5.4 Mandatory MTP Data Model (Runtime Structure)
MTP is mandatory from v1 as a conceptual runtime data model. The implementation is not required to provide full MTP package tooling, but the OPC UA object organization shall include MTP-aligned concepts.

### 5.4.1 Required Top-Level Objects
Under `Objects/MarsBase/AirlockPEA`, provide:
1. `PEAInformationLabel`
2. `Diagnostics`
3. `ServiceSet`
4. `Simulation`
5. `FaultInjection`

### 5.4.2 ServiceSet Structure
`ServiceSet/AirlockService` shall include:
1. `ServiceInformation`
2. `Modes`
3. `StateMachine`
4. `Procedures`
5. `DataAssemblies`

### 5.4.3 Modes and State Machine
1. `Modes` shall include:
- `OperationMode` (`OFF`, `AUTO`, `MANUAL`, `MAINT`)
- `SourceMode` (`OPERATOR_UI`, `REMOTE_OPCUA`, `SYSTEM_AUTO`)
- `CommandEn` (Boolean)
- `CommandEnReason` (String)

2. `StateMachine` minimum states:
- `idle`, `starting`, `execute`, `completing`, `completed`
- `pausing`, `paused`, `resuming`
- `holding`, `held`, `unholding`
- `stopping`, `stopped`
- `aborting`, `aborted`
- `resetting`

### 5.4.4 Procedures
Required procedures:
1. `Proc_DepressurizeForEVA`
2. `Proc_PressurizeForEntry`
3. `Proc_ManualDoorJog`

Each procedure shall expose:
1. `Parameters`
2. `Status`
3. `ReportValues`

### 5.4.5 DataAssemblies
Provide three groups:
1. `Indicators`:
- Pressure, temperature, O2, door positions, door closed bits.
- Pump and valve current status.
- Alarm summary and out-of-spec indicator.

2. `Parameters`:
- Unlock limits, alarm limits, leak nominal, control enable flags.

3. `ActiveElements`:
- `VentValve`
- `EqualizeValve`
- `VacuumPumpDrive`

`DataAssemblies/Control` shall host Operator/Remote request channels and `ActiveCommand` telemetry.

## 5.5 OPC UA Diagnostics Interface Requirements
Diagnostics nodes shall include:
1. Server uptime and start time.
2. Endpoint and active security modes.
3. Connected client count and per-client metadata.
4. Subscription count and publish rate.
5. Last error and last rejected command.

The diagnostics data shall be sufficient for UAExpert users to troubleshoot endpoint/security/session issues.

## 5.6 Web UI Requirements
The UI shall provide all three interfaces in one application, either tabbed or multi-pane.

### 5.6.1 Interface A: OPC UA Connectivity and Diagnostics
Must display:
1. Endpoint URL and copy action.
2. Active security profile.
3. Server certificate fingerprint/status.
4. Connected clients and last activity.
5. Subscription and publish statistics.
6. Last rejected command and reason.
7. Quick browse list of critical node paths.

### 5.6.2 Interface B: 3D Real-Time Airlock View
Must display and animate:
1. Habitat, chamber, and exterior scene.
2. Inner and outer door positions.
3. Pressure-state visual cues (venting/fog/particles).
4. Valve and pump state overlays.
5. Current service/command state banner.
6. Out-of-spec highlighting.

The view shall update from backend snapshots and reflect OPC UA command effects.

### 5.6.3 Interface C: Fendt-Themed HMI + P&ID (Hard Requirement)
This style is mandatory, not optional.

Must include:
1. Fendt-style gauge panel for pressure, temperature, O2, valve positions, pump load.
2. Prominent state and command strip with timestamps.
3. Alarm and event panel with severity.
4. P&ID mimic of habitat, valves, chamber, doors, pump, flow direction.
5. Blocking-condition text for rejected/held commands.

Theme constraints:
1. Visual language must be clearly Fendt-inspired industrial HMI.
2. Typography, color system, and iconography must be coherent across dashboard and P&ID.
3. Responsive behavior required for desktop and tablet widths.

## 5.7 Backend-Frontend WebSocket Contract
1. UI receives periodic snapshots, target default `10 Hz`.
2. Snapshot payload minimum fields:
- `timestamp`
- physics telemetry
- door/lock/valve/pump state
- service state and active command
- alarm list
- diagnostics summary

3. UI-originated commands shall still be written into OPC UA `Commands/Operator/Req/*` via backend command path, not bypass logic.

## 6. Concrete OPC UA Browse Tree (v1)
```
Objects
└─ MarsBase
   └─ AirlockPEA
      ├─ PEAInformationLabel
      ├─ Diagnostics
      ├─ ServiceSet
      │  └─ AirlockService
      │     ├─ ServiceInformation
      │     ├─ Modes
      │     ├─ StateMachine
      │     ├─ Procedures
      │     │  ├─ Proc_DepressurizeForEVA
      │     │  ├─ Proc_PressurizeForEntry
      │     │  └─ Proc_ManualDoorJog
      │     └─ DataAssemblies
      │        ├─ Indicators
      │        ├─ Parameters
      │        ├─ ActiveElements
      │        └─ Control
      │           ├─ OperatorCommands
      │           │  ├─ Req
      │           │  └─ Rsp
      │           ├─ RemoteCommands
      │           │  ├─ Req
      │           │  └─ Rsp
      │           └─ ActiveCommand
      ├─ Simulation
      └─ FaultInjection
```

## 7. Nonfunctional Requirements
1. Determinism: identical replay inputs produce identical state traces.
2. Availability: backend shall recover from client disconnect/reconnect without restart.
3. Observability: all command decisions are auditable by timestamp and source.
4. Performance targets:
- Simulation loop jitter under `5 ms` at default timestep on target hardware.
- OPC UA telemetry update cycle suitable for `10 Hz` UI and PLC monitoring.
- UI render target `>= 30 FPS` on reference desktop.

## 8. Acceptance Criteria
## 8.1 OPC UA Interoperability (Priority 1)
1. UAExpert can browse full namespace under `Objects/MarsBase/AirlockPEA`.
2. Writable request commands from both channels are accepted/rejected deterministically.
3. `ActiveCommand` accurately reflects source, command, state, and progress.
4. Diagnostics show connected clients, subscriptions, and recent errors.
5. Security profile selection works at startup and is visible in diagnostics.

## 8.2 Simulation Behavior
1. Interlocks always enforced.
2. Pressure trajectory is stable and directionally correct for valve/pump actions.
3. Fault injection affects telemetry and command acceptance as specified.

## 8.3 UI Completeness
1. All three interfaces exist and update live.
2. 3D state matches backend and command effects.
3. Fendt-themed dashboard and P&ID are present and operational.
4. Alarm and blocking reasons are visible and understandable.

## 9. Implementation Sequence
1. Build deterministic simulation core and command validator.
2. Implement OPC UA namespace and writable command handshake.
3. Implement MTP runtime object structure.
4. Add diagnostics and client/session telemetry.
5. Build WebSocket snapshots.
6. Build UI panels in order: Diagnostics, HMI/P&ID, 3D.
7. Run interoperability tests in UAExpert and PLC test harness.

## 10. Open Implementation Notes
1. NodeIds and BrowseNames must be stable once published.
2. Keep names ASCII and space-free.
3. Version enums and namespace changes explicitly for backward compatibility.

## 11. Multi-PEA Addendum (February 27, 2026)
### 11.1 PEA = Subsystem = Endpoint
1. Adding a subsystem means adding a PEA package.
2. Each PEA package must include:
- runtime state-machine service
- MTP object-model metadata
- one dedicated OPC UA endpoint
- mapping definitions from internal tags to Data Assemblies

### 11.2 Required Artifacts
1. Canonical mapping YAML (platform-agnostic) for each PEA.
2. Generated browse-tree manifest listing endpoint, namespace, root path, required node families, and required browse paths.
3. Rust-side endpoint host abstraction for per-PEA OPC UA lifecycle.

Reference files:
1. `mars-airlock-sim/backend/spec/pea-canonical-mapping.yaml`
2. `mars-airlock-sim/backend/spec/generated/underhill-base-browse-tree.manifest.yaml`
3. `mars-airlock-sim/backend/src/pea_endpoint_host.rs`

### 11.3 ServiceControl Handshake (Per Service)
Each service must expose at minimum:
1. `Cmd`
2. `ReqId`
3. `AckId`
4. `State`
5. `ErrorCode`
6. `DiagText`

Execution rule:
1. Service acts only on new `ReqId` where `ReqId != AckId`.

### 11.4 Mapping Definition Requirements
Each mapping entry must define:
1. source URI (`tag://...`)
2. sink URI (`da://...`)
3. direction (`tag_to_da` or `da_to_tag`)
4. transform chain
5. cycle/update rate

Compiler outputs:
1. Rust mapping executor bindings
2. Optional PLC Structured Text mapping outputs
