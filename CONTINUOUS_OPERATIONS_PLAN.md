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

Checkpoint capture, state mutation, and state-bearing journal append share one plant transaction boundary. Transient connection diagnostics may update outside that boundary because they are explicitly reset on restore; physical, command, operator, maintenance, inventory, and scheduler state may not.

### 4.3 MTP and WinCC OA integration architecture

WinCC OA is treated as the Process Orchestration Layer (POL), with one independent OPC UA client connection per Process Equipment Assembly (PEA). Underhill therefore adopts the MTP/VDI-VDE-NAMUR 2658 pattern rather than exposing every subsystem through one central OPC UA gateway.

The target contract is:

- every PEA has a stable identity, independently addressable OPC UA endpoint, application URI, namespace URI, and application-instance certificate;
- disconnecting or restarting one PEA does not disconnect the other PEAs from WinCC OA;
- each PEA exposes the same generated MTP service state machine, command handshake, operation modes, alarms, diagnostics, and health structure;
- PEA-specific process variables extend that common structure without inventing a different control model for every subsystem;
- WinCC OA can discover endpoints and import generated metadata rather than depending on hand-maintained tag and endpoint lists;
- OPC UA is the northbound SCADA boundary, not the fast internal physics-coupling bus.

The initial monolithic runtime remains a useful kernel demonstrator, but it is a migration stage. The production topology moves toward one independently deployable process per PEA, plus plant-level services for deterministic scheduling, conserved-resource transactions, persistence coordination, discovery, historian ingestion, and authority fencing. Physics coupling between processes must preserve deterministic ordering and conservation; the IPC implementation will be selected through an architecture decision record and benchmark rather than embedded in subsystem code.

```text
WinCC OA / POL
  |-- OPC UA client --> Airlock PEA process
  |-- OPC UA client --> ECLSS PEA process
  |-- OPC UA client --> Sabatier PEA process
  |-- OPC UA client --> Power PEA process
  `-- ... 15+ independently supervised PEA processes

PEA processes <--> deterministic plant/resource coordination bus
       |                         |
       v                         v
local checkpoint/journal    plant conservation ledger
```

Implementation decisions and gates:

1. Define one version-controlled MTP-compatible base information model and generate standard NodeSet2 artifacts and tag manifests from it.
2. Use MTPPy as a behavioral/reference fixture, not as the production runtime.
3. Benchmark the actively maintained async Rust OPC UA stack against open62541 bindings for 15, 30, and 60 concurrent long-lived server instances. The gate covers NodeSet2 loading, subscriptions, reconnect behavior, certificate handling, memory, CPU, and shutdown isolation.
4. Add an OPC UA Local Discovery Server or equivalent discovery service so adding a PEA does not require editing a hard-coded WinCC OA endpoint list.
5. Automate issuance, trust distribution, rotation, revocation, and expiry monitoring for one certificate per PEA. Private keys never enter source control or checkpoints.
6. Run a WinCC OA qualification harness with separate connections, subscriptions, command handshakes, certificate rejection/renewal, individual PEA restart, network interruption, and reconnect-without-value-confusion tests.
7. Preserve stable namespace URIs and NodeIds across restart, software upgrade, and state migration. A PEA may be replaced; its identity must not silently change.

The reference inputs for this work are the published MTP 2658 family, OPC Foundation NodeSet2 and discovery tooling, the MTPPy reference implementation, and NASA ECLSS reliability data. Third-party library selection remains provisional until license, maintenance status, interoperability, and soak-test results are recorded in ADRs.

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

External calibration and validation sources are governed by [REFERENCE_DATASETS.md](REFERENCE_DATASETS.md). That catalog separates direct physical evidence from diagnostic fixtures, security corpora, and structural analogs so domain-mismatched data cannot silently become Mars-base physics.

## 8. Degradation and maintenance

Maintainable components carry operating hours, cycles, environment exposure, efficiency, health, estimated remaining life, inspection interval, failure modes, required isolation, tools, spares, repair duration, and verification criteria.

Examples include scrubber saturation/regeneration, filter loading, catalyst deactivation/regeneration/replacement, battery capacity and resistance change, pump and valve wear, seal leakage, radiator dust accumulation, sensor calibration drift, and biofilm growth.

Maintenance occurs while the plant continues operating. Redundancy, reduced capacity, deferred work, spare allocation, robotic work limits, and post-maintenance validation are part of the plant state.

### 8.1 ECLSS reliability data pipeline

ICES-2025-127 and its cited ISS Maintenance and Analysis Data Set (MADS) work are candidate calibration sources for OGS, CDRA, UPA, WPA, and Sabatier/CRS reliability. Before numbers enter the simulator, Underhill will preserve the source revision, population and exposure basis, component mapping, censoring assumptions, confidence bounds, and any transformation from reported MTBF/MTTR to a sampled hazard or repair-duration distribution. Published aggregate statistics must not be presented as raw MADS records, and ISS-era hardware values must remain configurable rather than being asserted as exact future Mars-base performance.

The first reliability-data artifact will be a reviewed machine-readable component table plus a provenance note and tests showing that accelerated-sol fault campaigns reproduce the configured rates within statistical tolerance. Missing or weakly supported values are explicitly marked estimated, with sensitivity ranges, instead of being filled with untraceable constants.

## 9. Telemetry architecture

Each canonical tag declares stable ID, owner PEA, value type, engineering unit, range, physical source, cadence, criticality, quality semantics, retention class, alarm limits, and whether it is internal truth, sensed, derived, commanded, or diagnostic.

The catalog generates protocol surfaces and prevents OPC UA, UNS, i3X, HTTP, UI, and documentation from drifting apart.

“Datapoint” is not used as an ambiguous capacity unit. Underhill separately counts hidden physical state variables, canonical interface tags, active SCADA monitored items, emitted samples per second, alarm/events per second, and historian records. Multiple WinCC OA or agent subscriptions to one tag increase monitored-item and notification load but do not create new canonical tags.

Target canonical interface scale:

| Milestone | Process variables | Total interface tags |
| --- | ---: | ---: |
| Current prototype | 40-60 | about 100 |
| Continuous-kernel demonstrator | about 250 | about 400 |
| Survival spine | about 1,150 | about 1,800 |
| Minimum realistic autonomous outpost | 5,000-8,000 | 12,000-25,000 |
| Fully formed high-fidelity base | 25,000-50,000 | 60,000-120,000 |
| Research-grade component digital twin | 100,000+ | 250,000+ |

The fully formed target is budgeted across subsystem families as follows. These are interface tags, not all fast-published values:

| Subsystem family | Target tags | Examples included |
| --- | ---: | --- |
| Mars environment and site services | 5,000 | weather, dust, radiation, terrain/site sensors, external boundaries |
| Power generation, storage, and DC grid | 14,000 | cells/modules, strings, converters, buses, breakers, protection, quality, commands |
| Thermal control and heat rejection | 10,000 | loops, branches, pumps, valves, exchangers, radiators, temperatures and flows |
| Habitat atmosphere and ECLSS | 14,000 | compartments, gas species, ventilation, scrubbers, O2, trace contaminants, crew loads |
| Water recovery, storage, and waste | 10,000 | tanks, treatment stages, quality, dosing, brine, hygiene and waste flows |
| Safety, pressure structure, and radiation | 8,000 | fire/toxic gas, suppression, seals, hatches, structural and radiation monitoring |
| Gas handling and Sabatier | 7,000 | storage, compressors, reactants, reactor trains, catalysts and products |
| ISRU, manufacturing, and material closure | 9,000 | intake, separation, excavation, processing, fabrication and inventories |
| Agriculture, food, and bioprocessing | 8,000 | growth zones, lighting, nutrients, climate, biomass and food stocks |
| Robotics, logistics, and maintenance | 8,000 | robot joints/health, work cells, tools, work orders, spares and inspections |
| EVA, mobility, airlocks, and suits | 4,000 | airlocks, suit consumables, rovers, charging, navigation and EVA state |
| Communications, timing, compute, and cyber | 5,000 | links, clocks, compute health, OPC UA diagnostics, authority and security events |
| Cross-plant diagnostics and conservation | 8,000 | mass/energy ledgers, sensor residuals, KPIs, forecasts and model confidence |
| **Total planning envelope** | **110,000** | adjustable as PEA designs become concrete |

Implementation status: schema version 1 of this 110,000-tag catalog is now generated and validated at backend startup. Stable tag IDs and complete metadata are discoverable through paginated HTTP APIs, and capacity statistics distinguish the roughly 39,089 nominal publications per second from the 1.1 million samples per second implied by a naive all-tags-at-10-Hz design. Catalog presence is not counted as model-backed activation; each PEA must explicitly bind definitions to conserved state, sensors, quality evolution, commands, and alarms. Fifteen initial ECLSS/Power/Water/Safety tags are currently marked `model_backed` and archived at deterministic one-second plant-time boundaries with quality and provenance. A tag is promoted to `sensed` only after its observation/noise/fault path is distinct from internal truth.

At 110,000 tags, naive 10 Hz publication would produce 1.1 million samples per second and is neither realistic nor useful. The catalog assigns each tag an internal integration cadence, sensing cadence, publication class, deadband, event behavior, and retention policy. Fast protection and control values may run at 20-100 Hz internally; ordinary SCADA values commonly publish at 1 Hz or on change; inventory, wear, and forecast values publish much more slowly. WinCC OA receives operationally meaningful tags while high-rate component truth and waveforms can remain in specialized streams or validation forks.

Publication cadence is tag-specific. Fast internal integration never requires broadcasting every tag at the same rate.

For SCADA sizing, the catalog also records the PEA endpoint and WinCC OA subscription class. Capacity tests use at least 15 independent OPC UA sessions—not one aggregated session—and include reconnect bursts, monitored-item recreation, alarm/event load, and certificate operations. Tests progress through 5,000, 25,000, 100,000, and 250,000 canonical tags, while allowing survival-critical PEAs such as power and ECLSS to carry substantially more than the average. Connection count, monitored-item count, notification rate, and historian write rate are separate budgets.

## 10. Agent and controller contract

Agents join an already-running plant. The normal interface has no episode reset.

Agents receive discoverable capabilities, timestamped telemetry with quality, history, alarms, events, resource forecasts, work orders, staged command receipts, and scoped authority leases. High-risk actions may require deterministic preconditions or human approval.

Agents never bypass the safety kernel. Hot-swapping an agent or controller must not stop the plant. Qualification uses checkpoint-derived forks with the same command and telemetry interfaces as the live plant.

Implementation status: the live PEA set now has a validated shared identity registry. Primary PEA discovery, i3X object composition and relationships, service lookup, UNS announcement identity, and OPC UA namespace metadata resolve from that contract. This removes a previously observed split-brain condition in which the primary API exposed six PEAs while the lower-level i3X object graph exposed only three. Subsystem operator-state mutation is now shared across every non-Airlock PEA, and command-disable plus OFF/MAINT are enforced as lifecycle-start interlocks and reconciled on restore. Runtime storage and the remaining lifecycle dispatch are still specialized per PEA and remain the next consolidation boundary.

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

In parallel, freeze stable PEA identities, endpoint allocation, namespace rules, and certificate boundaries so persistence does not accidentally encode the current monolithic deployment as the permanent architecture.

### Phase 2: Conserved resource kernel

Implement typed resource stocks/flows, component ports, transaction ordering, reconciliation, invariants, and historian records.

### Phase 3: Survival spine

Implement Power, Thermal, Habitat/ECLSS, Water/Waste, and Safety/Structure PEAs with cross-system constraints and reference-controller behavior.

Each survival-spine PEA graduates to an independently restartable OPC UA server process with the common MTP contract before the phase is complete.

### Phase 4: Continuous stressors

Implement environment, demand schedules, fault generation, degradation, maintenance, spares, and robotic work execution.

### Phase 5: Industrial closure

Implement gas distribution, Sabatier, hydrogen recovery, ISRU, agriculture, waste processing, mobility, and logistics.

### Phase 6: Agent validation and live operation

Implement constrained authority, fork orchestration, continuous evaluation, model/data provenance, long-run dashboards, soak campaigns, and zero-discontinuity deployment.

Initial implementation: a checkpointed plant-time campaign runner schedules bounded faults without
starting or ending the plant, accepts agent observations, and scores root-cause accuracy, simulated
detection latency, and confidence. Airlock valve-stiction and Safety compound leak/fire templates
are live. Dataset-derived trace replay, safe-action scoring, fork orchestration, and long-run campaign
aggregation remain follow-on work.

## 14. Immediate implementation sequence

The first incremental changes are:

1. Introduce a tested plant clock and deterministic multi-rate scheduler while preserving the existing 50 ms fixed physics step.
2. Expose plant elapsed time, Mars sol, time scale, scheduler step, and catch-up backlog through health telemetry.
3. Add accelerated-operation configuration without increasing protocol publication rates uncontrollably.
4. Define the first versioned checkpoint envelope and full-state serialization boundary.
5. Add atomic checkpoint writing and restart restoration.
6. Add append-only journal ordering and replay.
7. Move hard-coded PEAs behind a common runtime contract.
8. Publish an ADR and benchmark harness for the Rust OPC UA stack at 15+ independent PEA endpoints.
9. Define and generate the common MTP NodeSet2 model, stable namespace/NodeId rules, discovery registration, and per-PEA certificate lifecycle.
10. Add WinCC OA multi-connection soak and individual-PEA restart/reconnect tests.

Persistence is not considered complete until all physical, sensor, command, maintenance, scheduler, and random-generator state survives restart.
