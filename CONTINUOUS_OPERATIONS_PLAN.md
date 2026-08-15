# Underhill Continuous Operations Plan

## 1. Objective

Underhill is a persistent autonomous continuous-process Mars base simulator. It is not a finite mission, an episode, or a scenario with a terminal success state.

The base is created once and then operates indefinitely:

```text
operate -> consume -> recover -> regenerate -> degrade -> maintain -> repair -> operate
```

Individual PEAs and procedures may start, complete, stop, fail, isolate, regenerate, or be replaced. The plant itself has no `MISSION_COMPLETE`, routine reset, or normal end state.

The simulator exists to provide a realistic live validation environment for agents, industrial controllers, SCADA/POL systems, and other developed systems. New controllers must be able to join a plant that already has history, depleted inventories, active work, degraded equipment, and imperfect sensors.

## 2. Non-negotiable invariants

1. **Continuous identity**: restart or deployment must not create a new nominal plant.
2. **Deterministic physics**: the same checkpoint, journal, configuration, and random seed must reproduce the same plant trajectory.
3. **Conservation**: mass, energy, and inventory transfers reconcile within declared numerical tolerances.
4. **No hidden infinity**: consumables, storage, spares, sorbents, filters, catalysts, and waste capacity are finite unless a configuration explicitly declares an external source or sink.
5. **Truth is not telemetry**: agents normally observe sensor outputs with timestamp, quality, delay, noise, drift, and possible faults rather than perfect internal state.
6. **Safety is deterministic**: hard interlocks and authority boundaries are enforced below the agent/controller layer.
7. **Component lifecycle, not plant lifecycle**: deploy/start/stop/repair/replace applies to equipment; the base never transitions to a terminal state.
8. **One canonical writer**: only one runtime may advance or command the canonical plant at a time.
9. **Forks are isolated**: reset and rewind are permitted only for checkpoint-derived validation forks.
10. **Every run is auditable**: commands, faults, configuration changes, maintenance, handoffs, and model versions are journaled.

## 3. Operating model

### 3.1 Time modes

One model supports three modes:

- **Live**: plant time advances with wall time.
- **Accelerated**: fixed physics steps execute faster than wall time for long-horizon qualification.
- **Forked validation**: an isolated plant starts from a canonical checkpoint and can be discarded without changing the live plant.

Plant time is monotonic and independent of host clock corrections. Wall-clock timestamps remain metadata for external correlation. The runtime records elapsed plant seconds, Mars sol index, sol fraction, model step, and configured time scale.

### 3.2 Downtime policy

The canonical policy is catch-up, not silent freezing: if the process is unavailable while the represented base should continue operating, restart loads the last durable state and deterministically advances through the gap using the autonomous controller, schedules, degradation, and journaled events.

Catch-up must be bounded and explicit. If the gap cannot be reconstructed safely, the runtime enters a `RECOVERY_REQUIRED` operating condition rather than inventing unobserved control actions. Development forks may choose a freeze policy.

### 3.3 Multi-rate deterministic scheduling

The simulation uses a single ordered scheduler with several cadences:

- **Fast** (50 ms default): pressure transients, valves, drives, hatch motion, electrical protection.
- **Medium** (1 s default): power balance, thermal loops, ventilation, ordinary controllers.
- **Slow** (60 s default, with hourly/sol tasks derived from it): inventories, consumables, fouling, wear, dust, maintenance, reliability, agriculture.

Network, storage, and protocol I/O may be asynchronous. Physical state advancement remains deterministically ordered.

## 4. Target architecture

```text
environment and schedules
          |
          v
  deterministic plant scheduler
          |
          v
 resource network <-> PEA physical models <-> maintenance/degradation
          |                    |
          |                    v
          |             hidden physical truth
          |                    |
          v                    v
 conservation audit      sensor models
                               |
                               v
                    OPC UA / UNS / i3X / HTTP
                               |
                               v
                     controllers and agents
                               |
                               v
                 authority + deterministic safety kernel
                               |
                               v
                         actuator models
```

### 4.1 Common PEA contract

Each subsystem eventually implements a common contract for:

- definition and stable identity;
- declared resource ports;
- process and sensor state;
- commands, procedures, and staged writeback;
- fast/medium/slow updates;
- alarms and invariants;
- checkpoint serialization and migration;
- maintenance actions and replaceable components;
- generated OPC UA, UNS, i3X, and documentation mappings.

### 4.2 Persistence

Persistence is split by purpose:

- **Operational state store**: versioned atomic checkpoints, active plant lease, schema migrations.
- **Append-only journal**: commands, acknowledgements, faults, maintenance, schedules, configuration changes, controller handoffs.
- **Historian**: high-volume telemetry with retention, downsampling, and Parquet/time-series export.

Restart loads the checkpoint, replays the journal after its position, validates invariants, acquires the writer lease, and then resumes advancement.

## 5. Resource economy

The plant carries explicit stocks and flows for:

- electrical and thermal energy;
- O2, N2, CO2, H2, CH4, water vapor, and trace gases;
- potable, hygiene, process, condensate, wastewater, brine, and stored water;
- food, nutrients, biomass, solid waste, and recoverable material;
- filters, sorbents, catalyst, suppression media, lubricants, and other consumables;
- spare parts, tools, robotic work capacity, and maintenance backlog.

Every integration interval reconciles:

```text
opening inventory
+ produced and received
- consumed, transferred, leaked, vented, rejected, and wasted
= closing inventory
```

External resources such as Mars atmosphere, subsurface water, solar flux, fission fuel, and cargo deliveries are explicit configurable boundaries. Resupply is optional, never assumed.

## 6. Subsystem build order

### 6.1 Survival spine

1. **Power generation, storage, and grid**: generation, buses, converters, breakers, batteries, protection, load shedding, black start, degradation.
2. **Thermal control**: coolant branches, pumps, valves, exchangers, heat loads, radiators, freeze/overheat/leak states.
3. **Habitat and ECLSS**: compartment gas masses and partial pressures, crew-equivalent loads, ventilation, CO2 bed cycling, O2 generation, humidity condensation, trace contaminants, leaks, isolation.
4. **Water and waste**: tanks, collection, recovery, quality verification, reject/brine, fouling, microbial risk, emergency reserves.
5. **Safety and structure**: fire/toxic gas, suppression, pressure boundaries, seals, radiation shelter, dust ingress, alarm voting.

### 6.2 Industrial closure

6. Gas storage and distribution.
7. Sabatier and hydrogen recovery with conserved reactants/products.
8. Mars atmosphere intake and ISRU.
9. Robotic logistics and maintenance.
10. Food storage, controlled agriculture, and solid-waste processing.
11. External mobility, inspection, communications, and positioning/time services.

## 7. Environment and continuous demand

Static scenarios are replaced by continuous drivers:

- diurnal and seasonal external pressure and temperature;
- solar flux, radiation, wind, dust opacity, and dust deposition;
- statistically generated dust storms and rare environmental events;
- deterministic human-equivalent metabolic, water, waste, heat, food, occupancy, and EVA loads;
- robotic operations, inspection, logistics, and maintenance schedules;
- reliability events using both memoryless hazards and age/load/condition-dependent failure models.

Reference climate and consumption datasets are versioned inputs. Their provenance and uncertainty are recorded with each model release.

## 8. Degradation and maintenance

Maintainable components carry operating hours, cycles, environment exposure, efficiency, health, estimated remaining life, inspection interval, failure modes, required isolation, tools, spares, repair duration, and verification criteria.

Examples include scrubber saturation/regeneration, filter loading, catalyst deactivation/regeneration/replacement, battery capacity and resistance change, pump and valve wear, seal leakage, radiator dust accumulation, sensor calibration drift, and biofilm growth.

Maintenance occurs while the plant continues operating. Redundancy, reduced capacity, deferred work, spare allocation, robotic work limits, and post-maintenance validation are part of the plant state.

## 9. Telemetry architecture

Each canonical tag declares stable ID, owner PEA, value type, engineering unit, range, physical source, cadence, criticality, quality semantics, retention class, alarm limits, and whether it is internal truth, sensed, derived, commanded, or diagnostic.

The catalog generates protocol surfaces and prevents OPC UA, UNS, i3X, HTTP, UI, and documentation from drifting apart.

Target scale:

| Milestone | Process variables | Total interface tags |
| --- | ---: | ---: |
| Current prototype | 40-60 | about 100 |
| Continuous-kernel demonstrator | about 250 | about 400 |
| Survival spine | about 1,150 | about 1,800 |
| Realistic autonomous base | about 3,300 | 4,600-5,300 |
| High-detail digital twin | 8,000+ | 12,000+ |

Publication cadence is tag-specific. Fast internal integration never requires broadcasting every tag at the same rate.

## 10. Agent and controller contract

Agents join an already-running plant. The normal interface has no episode reset.

Agents receive discoverable capabilities, timestamped telemetry with quality, history, alarms, events, resource forecasts, work orders, staged command receipts, and scoped authority leases. High-risk actions may require deterministic preconditions or human approval.

Agents never bypass the safety kernel. Hot-swapping an agent or controller must not stop the plant. Qualification uses checkpoint-derived forks with the same command and telemetry interfaces as the live plant.

## 11. Deployment without discontinuity

State schemas and canonical tags are versioned from the beginning. Deployment supports:

- checkpoint compatibility validation;
- forward state migrations with tested rollback rules;
- a single-writer lease and fencing token;
- old-instance drain and new-instance handoff;
- telemetry continuity and explicit gap markers;
- rollback without silently reverting physical plant history.

## 12. Validation and rolling evaluation

Correctness is measured continuously rather than by mission completion:

- hard safety-envelope violations;
- mass, energy, and inventory reconciliation error;
- time in degraded/emergency states;
- resource margins and projected depletion;
- recovery time and control stability;
- maintenance backlog and overdue critical work;
- invalid/rejected commands and unnecessary cycling;
- alarm detection quality and human interventions;
- historian gaps, stale telemetry, and sensor disagreement;
- memory, storage, latency, and scheduler backlog.

Qualification gates:

1. Deterministic replay from checkpoint plus journal.
2. Crash/restart with operational continuity.
3. 30, 365, and 1,000 accelerated sols without global reset.
4. Bounded conservation error for every modeled resource.
5. Cascading and simultaneous fault campaigns.
6. 72-hour wall-clock soak.
7. Agent/controller handoff without plant shutdown.
8. State migration and blue/green single-writer handoff.

## 13. Delivery phases

### Phase 0: Repository baseline

Make a clean clone reproducible, remove generated artifacts from source control, establish CI, reconcile specifications, and publish one-command startup and validation paths.

### Phase 1: Continuous runtime

Implement plant time, deterministic multi-rate scheduling, live/accelerated/fork modes, runtime observability, checkpoint state schema, journal, restore, downtime policy, and state migrations.

### Phase 2: Conserved resource kernel

Implement typed resource stocks/flows, component ports, transaction ordering, reconciliation, invariants, and historian records.

### Phase 3: Survival spine

Implement Power, Thermal, Habitat/ECLSS, Water/Waste, and Safety/Structure PEAs with cross-system constraints and reference-controller behavior.

### Phase 4: Continuous stressors

Implement environment, demand schedules, fault generation, degradation, maintenance, spares, and robotic work execution.

### Phase 5: Industrial closure

Implement gas distribution, Sabatier, hydrogen recovery, ISRU, agriculture, waste processing, mobility, and logistics.

### Phase 6: Agent validation and live operation

Implement constrained authority, fork orchestration, continuous evaluation, model/data provenance, long-run dashboards, soak campaigns, and zero-discontinuity deployment.

## 14. Immediate implementation sequence

The first incremental changes are:

1. Introduce a tested plant clock and deterministic multi-rate scheduler while preserving the existing 50 ms fixed physics step.
2. Expose plant elapsed time, Mars sol, time scale, scheduler step, and catch-up backlog through health telemetry.
3. Add accelerated-operation configuration without increasing protocol publication rates uncontrollably.
4. Define the first versioned checkpoint envelope and full-state serialization boundary.
5. Add atomic checkpoint writing and restart restoration.
6. Add append-only journal ordering and replay.
7. Move hard-coded PEAs behind a common runtime contract.

Persistence is not considered complete until all physical, sensor, command, maintenance, scheduler, and random-generator state survives restart.
