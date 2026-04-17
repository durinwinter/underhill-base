# WinCC OA POL Integration Guide

## Purpose

This document defines how the Underhill Mars Base simulator should behave when WinCC OA is acting as the Process Orchestration Layer (POL) over the PEA set.

The immediate design target is not perfect plant physics. The target is realistic control-system behavior:
- writes do not mutate process state instantly
- accepted commands pass through a writeback lifecycle
- process values settle after a control action, not in the same cycle
- commandability depends on deployment, run state, mode, and authority
- subsystem interactions propagate across PEAs rather than staying isolated

## Current Runtime Model

The current simulator exposes three active PEAs:
- `AIRLOCK-PEA-001`
- `ECLSS-PEA-001`
- `SABATIER-PEA-001`

Airlock already uses a request/response command handshake with explicit arbitration.

ECLSS and Sabatier now use staged subsystem writebacks through `POST /api/v1/pea/{pea_id}/advanced-command`.

### Subsystem Writeback Contract

For `ECLSS-PEA-001` and `SABATIER-PEA-001`, WinCC OA should treat the command path as asynchronous.

Returned writeback states:
- `PENDING`: command accepted by the POL-facing API, waiting for apply delay
- `APPLIED`: command has crossed the control boundary and actuator/setpoint state changed
- `SETTLING`: process values are converging toward the requested operating point
- `COMPLETE`: requested state reached within tolerance
- `REJECTED`: command blocked by mode, authority, runtime state, or invalid command
- `TIMED_OUT`: write accepted and applied, but the process did not settle inside the configured timeout

The API now returns both:
- `writeback.active`
- `writeback.last_completed`

The same writeback object is also exposed from subsystem operator-state endpoints and PEA descriptors.

## WinCC OA POL Assumptions

Assume WinCC OA is the authority that:
- sequences services across multiple PEAs
- owns higher-level mission procedures
- writes procedure requests and setpoints into the individual PEA surfaces
- consumes explicit writeback confirmation before issuing dependent steps

Recommended orchestration pattern:
1. Query the PEA descriptor or operator-state endpoint.
2. Confirm `deployed`, `running`, `command_en`, and source authority.
3. Issue one subsystem command.
4. Wait for `writeback.active.stage` to move through `PENDING` and `SETTLING`.
5. Only continue the mission sequence after `COMPLETE`.
6. Treat `TIMED_OUT` as degraded execution, not a silent success.

## Higher-Fidelity Priorities

### 1. Replace single-pole lags with engineering state-space models

Near-term gaps in the current simulator:
- no explicit valve actuator travel model for ECLSS/Sabatier
- no transport delay or dead time between write acceptance and process response beyond configured writeback delay
- no multi-node mass/energy inventory across habitat, buffer tanks, gas cleanup, and ISRU loops
- no stochastic sensor noise, drift, or calibration offset
- no equipment wear model except a simplified Sabatier catalyst health term

### 2. Make subsystem coupling explicit

The highest-value coupling additions are:
- Airlock to ECLSS: habitat pressure cycling, gas reclaim inventory, O2/CO2 transient after EVA
- ECLSS to Sabatier: CO2 availability, H2 recycle efficiency, water return
- Power/Thermal future PEAs to both: load limits, thermal derating, degraded startup windows

### 3. Model writebacks as actuator and controller behavior

For WinCC OA, the most useful realism is usually control realism:
- actuator travel time
- command queue depth
- permissives/interlocks
- stale-value detection
- source arbitration
- timeout/retry behavior
- bad-quality telemetry during transitions
- final-value tolerances instead of exact equality

## Recommended Library Stack

I infer from the current Rust codebase that the most useful additions are the following.

### Core physics and numerical solving

- `uom`
  - Use for type-safe engineering units so pressure, mass flow, power, and temperature do not cross in plain `f64` form.
- `nalgebra`
  - Use for state-space models, observers, mixing calculations, and coupled habitat/ISRU matrices.
- `ode_solvers` or `diffsol`
  - Use when ECLSS and Sabatier move beyond first-order approximations into coupled ODE systems.
- `fmi`
  - Use if you want the Rust backend to co-simulate imported FMUs from Modelica, Simulink, or other FMI-capable tools.

### Control logic and orchestration realism

- `smlang`
  - Use to make service/procedure/writeback state machines explicit and auditable.
- `petgraph`
  - Use to represent cross-PEA dependencies, propagation paths, and mission procedure graphs.

### Uncertainty, fault injection, and degraded modes

- `rand_distr`
  - Use for sensor noise, drift injection, start-delay distributions, and intermittent component faults.
- `statrs`
  - Use when you need statistical thresholding, confidence windows, or richer fault model distributions.

## Suggested Adoption Order

### Phase 1
- `uom`
- `smlang`
- `rand_distr`

This gives immediate value with minimal architecture risk.

### Phase 2
- `nalgebra`
- `ode_solvers` or `diffsol`

This is the point where the subsystem models become materially more realistic.

### Phase 3
- `fmi`
- `petgraph`

This is the right point if you want imported engineering models and multi-PEA mission graph orchestration.

## Writeback Fidelity Roadmap

### Near-term
- add per-command quality bits and command age
- expose estimated settle time and tolerance band in writeback responses
- add actuator position states separate from setpoints
- add source-lock / ownership tokens for POL vs local operator authority

### Mid-term
- add sensor bad-quality during startup/shutdown and transition deadbands
- add explicit controllers for pressure, CO2 removal, O2 generation, and reactor setpoint tracking
- add command journaling with replay support for deterministic regression tests

### Long-term
- import external FMUs for ECLSS and Sabatier process cores
- split the habitat into coupled control volumes instead of single averaged states
- co-simulate future PowerGrid, ThermalControl, Water, and HabitatStructure PEAs

## Practical WinCC OA Guidance

For WinCC OA MCP agents, assume:
- a command is not successful when it is accepted
- `COMPLETE` is the success boundary
- `TIMED_OUT` is a degraded branch that still requires operator or orchestration logic
- airlock is currently the strongest MTP/PackML-like implementation
- ECLSS and Sabatier are the best candidates for next fidelity investment because they drive the most cross-PEA dependencies
