# Underhill Mars Base Agent Brief

## Identity

Underhill is a modular Mars surface habitat simulator organized as a set of PEAs (Physical Equipment Assemblies) with one OPC UA endpoint per subsystem and a shared HTTP/UNS surface for orchestration, observability, and AI-assisted operations.

The simulator is intended for POL-driven operations, PLC/SCADA integration testing, MTP-style service modeling, and multi-PEA mission sequencing.

## Base Layout

Underhill is modeled as a three-tier base.

### Tier 1: Core life support and habitat
- Airlock
- ECLSS
- HabitatStructure
- Water

### Tier 2: Power, thermal, and ISRU
- PowerGen
- PowerGrid
- ThermalControl
- Sabatier

### Tier 3: Production, safety, and advanced operations
- Farm
- Safety
- NobleGasRecovery
- IsotopeFractionation
- AdvancedAtmospherePolishing
- GasStorageAndDistribution

Only the following PEAs are active in the current runtime:
- `AIRLOCK-PEA-001`
- `ECLSS-PEA-001`
- `SABATIER-PEA-001`

The remaining PEAs exist as cataloged future assets and should be treated as planned but not operational.

## Operational Architecture

Each active PEA has:
- its own OPC UA endpoint
- its own service/procedure model
- its own runtime state (`deployed`, `running`, `last_transition_ms`)
- its own commandability rules
- UNS publication into the Underhill namespace

Primary integration surfaces:
- HTTP API: `http://127.0.0.1:8080/api/v1/*`
- Airlock OPC UA: `opc.tcp://127.0.0.1:4841/underhill/airlock`
- ECLSS OPC UA: `opc.tcp://127.0.0.1:4842/underhill/eclss`
- Sabatier OPC UA: `opc.tcp://127.0.0.1:4843/underhill/sabatier`
- WebSocket snapshots: `/ws`

## PEA Roles

### Airlock PEA
Purpose:
- crew ingress/egress
- depressurization and repressurization
- hatch locking/unlocking
- atmosphere reclaim and isolation logic

Behavioral status:
- strongest command/state-machine implementation in the simulator
- uses explicit request/response handshakes and active command telemetry
- enforces hard interlocks on hatch state and pressure

Key hard interlocks:
- inner and outer hatches must not be open together
- outer hatch unlock is blocked above safe pressure
- inner hatch unlock is blocked below safe pressure
- pressure-related alarms and leak detection affect command acceptance

### ECLSS PEA
Purpose:
- cabin atmosphere revitalization
- CO2 removal
- oxygen generation
- humidity and cabin pressure conditioning
- water recovery
- branch isolation and safe-haven operation

Behavioral status:
- active dynamic simulation
- command path is staged as asynchronous writeback
- good target for higher-fidelity mass-balance and controller modeling

Important process values:
- cabin pressure
- O2 percent
- CO2 ppm
- humidity
- water recovery
- CO2 capture rate
- O2 generation rate
- power draw

### Sabatier PEA
Purpose:
- ISRU methanation using CO2 and hydrogen
- methane and water production
- catalyst regeneration
- emergency vent and safe shutdown behavior

Behavioral status:
- active dynamic simulation
- command path is staged as asynchronous writeback
- depends on ECLSS CO2 availability
- good target for reactor, thermal, and recycle-loop fidelity improvements

Important process values:
- reactor temperature
- reactor pressure
- CO2 feed
- H2 feed
- conversion efficiency
- methane production
- water production
- catalyst health
- power draw

## Command and Authority Model

Airlock uses explicit operator and remote request channels.

ECLSS and Sabatier use `POST /api/v1/pea/{pea_id}/advanced-command` and return staged writeback state.

Treat commandability as conditional on:
- `deployed == true`
- `running == true`
- subsystem `operation_mode` not in `Off` or `Maint`
- `command_en == true`
- source-specific authority enabled

Source meanings:
- `operator` or `operator_ui`: local operator authority
- `remote` or `remote_opcua`: remote control authority
- `pol`: mapped to operator-side orchestration authority in the current implementation

## Subsystem Writeback Model

For ECLSS and Sabatier, commands move through these stages:
- `PENDING`
- `APPLIED`
- `SETTLING`
- `COMPLETE`
- `REJECTED`
- `TIMED_OUT`

Interpretation:
- `PENDING` means accepted but not yet applied
- `APPLIED` means actuator/setpoint state changed
- `SETTLING` means process values are converging
- `COMPLETE` means the requested state reached tolerance
- `TIMED_OUT` means command applied but process did not settle in time

Agents should not treat `ACCEPTED` or `PENDING` as mission success.

## Cross-PEA Dependencies

Current modeled dependency:
- ECLSS CO2 capture availability feeds Sabatier CO2 intake potential

Operationally important future dependencies:
- Airlock EVA cycles affecting ECLSS atmosphere stabilization demand
- Power and thermal constraints affecting ECLSS and Sabatier startup/performance
- Water recovery and ISRU loops coupling back into habitat consumables management

## Recommended Agent Behavior

When acting through a WinCC OA MCP server or another orchestration agent:
1. Read the PEA descriptor first.
2. Confirm commandability and runtime state.
3. Issue one command at a time per subsystem.
4. Watch the writeback object until terminal state.
5. Branch on `COMPLETE`, `REJECTED`, or `TIMED_OUT` explicitly.
6. Do not infer success from setpoint writes alone.
7. Treat Tier 2 and Tier 3 catalog PEAs as planned assets unless runtime data says otherwise.

## Good Short Description For Other Agents

Underhill is a Mars habitat control simulator built around MTP-like PEAs. The currently operational PEAs are Airlock, ECLSS, and Sabatier, each with a separate OPC UA endpoint and shared HTTP/UNS interfaces. Airlock is the most complete state-machine implementation. ECLSS and Sabatier support asynchronous staged writebacks intended for POL-style orchestration, where command acceptance, application, and process settling are separate phases. The base is organized into Tier 1 habitat/life-support, Tier 2 power/thermal/ISRU, and Tier 3 production/safety/advanced systems, with many future PEAs already cataloged but not yet simulated in full.
