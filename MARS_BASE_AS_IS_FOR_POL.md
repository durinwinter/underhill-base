# Underhill Mars Base As-Is Reference For POL

Date baseline: March 13, 2026.

This document describes the Underhill Mars Base simulator as it exists now so a Process Orchestration Layer (POL), including a WinCC OA POL, can interact with it using the current runtime model rather than an assumed future architecture.

## 1. What Underhill Is Right Now

Underhill is a Mars habitat simulator organized around PEAs (Physical Equipment Assemblies). Each active PEA has its own OPC UA endpoint and participates in a shared HTTP and UNS surface.

The simulator currently behaves like this:
- Airlock is the most complete PEA and has the strongest command/state-machine implementation.
- ECLSS and Sabatier are actively simulated and exposed as PEAs, but their command model is different from Airlock.
- ECLSS and Sabatier now use staged writeback behavior for POL-facing commands.
- Many additional PEAs are cataloged in the base model, but they are not operational yet.

## 2. Current Operational PEAs

These are the only PEAs that should be treated as operational in the current runtime:
- `AIRLOCK-PEA-001`
- `ECLSS-PEA-001`
- `SABATIER-PEA-001`

These PEAs currently have stable endpoint identities in the simulator:
- Airlock OPC UA: `opc.tcp://127.0.0.1:4841/underhill/airlock`
- ECLSS OPC UA: `opc.tcp://127.0.0.1:4842/underhill/eclss`
- Sabatier OPC UA: `opc.tcp://127.0.0.1:4843/underhill/sabatier`
- HTTP API root: `http://127.0.0.1:8080/api/v1`
- WebSocket snapshots: `ws://127.0.0.1:8080/ws`

## 3. Cataloged But Not Operational PEAs

These PEAs exist in the current base catalog but should be treated as planned assets unless runtime data says otherwise:
- `HABITATSTRUCTURE-PEA-001`
- `WATER-PEA-001`
- `POWERGEN-PEA-001`
- `POWERGRID-PEA-001`
- `THERMALCONTROL-PEA-001`
- `FARM-PEA-001`
- `SAFETY-PEA-001`
- `NOBLEGASRECOVERY-PEA-001`
- `ISOTOPEFRACTIONATION-PEA-001`
- `ADVANCEDATMOSPHEREPOLISHING-PEA-001`
- `GASSTORAGEANDDISTRIBUTION-PEA-001`

POL implication:
- do not schedule mission logic against these unless the runtime starts returning them as deployed and running
- treat them as engineering placeholders for future expansion

## 4. Tier Structure Of The Base

### Tier 1: Core Life Support And Habitat
- Airlock
- ECLSS
- HabitatStructure
- Water

### Tier 2: Power, Thermal, And ISRU
- PowerGen
- PowerGrid
- ThermalControl
- Sabatier

### Tier 3: Production, Safety, And Advanced Operations
- Farm
- Safety
- NobleGasRecovery
- IsotopeFractionation
- AdvancedAtmospherePolishing
- GasStorageAndDistribution

## 5. What The POL Should Know Before Commanding

### Global assumptions
- `deployed` and `running` are meaningful for every active PEA.
- commandability is not guaranteed even if a PEA is running.
- ECLSS and Sabatier can reject commands based on mode and authority state.
- a command is not successful just because it was accepted by the API.
- the simulator uses a 50 ms deterministic simulation loop.

### Important current differences between PEAs
- Airlock uses a request/response command handshake with explicit command channels.
- ECLSS and Sabatier use `POST /api/v1/pea/{pea_id}/advanced-command`.
- `pol` currently maps to operator-side authority in subsystem command parsing.
- i3X writeback via `PUT /api/v1/objects/{element_id}/value` is still stubbed and returns `501`.

### Recommended POL discipline
1. Read the PEA descriptor or subsystem operator-state first.
2. Confirm `deployed`, `running`, and commandability.
3. Issue one command at a time per subsystem.
4. Wait for terminal writeback state before sequencing dependent work.
5. Treat `TIMED_OUT` as degraded execution, not success.

## 6. Active PEA Details

## 6.1 Airlock PEA

PEA ID:
- `AIRLOCK-PEA-001`

Purpose:
- crew ingress and egress
- chamber depressurization and repressurization
- hatch control and locking
- atmosphere reclaim and isolation behavior

Current strengths:
- strongest MTP-like command and state handling in the simulator
- explicit operator and remote request channels
- explicit response fields and active command tracking
- interlock-heavy behavior

Important Airlock services:
- `Depressurizing`
- `Pressurizing`
- `HatchTransfer`
- `AtmosphereReclaim`
- `EmergencyRepressurizing`
- `Isolation`

Important Airlock process values:
- chamber pressure
- temperature
- O2 percent
- inner door position
- outer door position
- equalize valve position
- vent valve position
- pump state and current
- alarms and out-of-spec status

Important Airlock interlocks:
- inner and outer doors must not be open at the same time
- outer unlock is blocked above the configured pressure threshold
- inner unlock is blocked below the configured pressure threshold

POL implication:
- Airlock is the best candidate for procedure-driven orchestration today
- if you need a reference PEA for MTP-like control semantics, use Airlock

## 6.2 ECLSS PEA

PEA ID:
- `ECLSS-PEA-001`

Purpose:
- CO2 removal
- oxygen generation
- water recovery
- atmosphere conditioning
- contingency life support actions

Current services:
- `Co2Removal`
- `OxygenGeneration`
- `WaterRecovery`
- `AtmosphereConditioning`
- `ContingencyLifeSupport`

Current command set:
- `START_CO2_SCRUBBER_BED_A`
- `STOP_CO2_SCRUBBER_BED_A`
- `SET_CO2_REMOVAL_FLOW_RATE`
- `START_ELECTROLYZER_STACK`
- `STOP_ELECTROLYZER_STACK`
- `SET_OXYGEN_GENERATION_RATE`
- `START_WATER_PROCESSOR`
- `STOP_WATER_PROCESSOR`
- `SET_WATER_RECOVERY_TARGET`
- `SET_CABIN_HUMIDITY_TARGET`
- `SET_CABIN_PRESSURE_TARGET`
- `ISOLATE_ECLSS_BRANCH`
- `RESTORE_ECLSS_BRANCH`
- `ENABLE_SAFE_HAVEN`
- `DISABLE_SAFE_HAVEN`

Important process values:
- `cabin_pressure_kpa`
- `cabin_pressure_target_kpa`
- `o2_percent`
- `co2_ppm`
- `humidity_pct`
- `humidity_target_pct`
- `water_recovery_pct`
- `water_recovery_target_pct`
- `co2_capture_kgph`
- `co2_flow_setpoint_kgph`
- `o2_generation_kgph`
- `o2_generation_setpoint_kgph`
- `power_kw`
- `co2_scrubber_running`
- `electrolyzer_running`
- `water_processor_running`
- `branch_isolated`
- `safe_haven_enabled`
- alarm flags for high CO2 and low O2

Current command authority model:
- governed by `operation_mode`
- governed by `command_en`
- governed by `operator_control_enabled`
- governed by `remote_control_enabled`

Current writeback behavior:
- `PENDING`
- `APPLIED`
- `SETTLING`
- `COMPLETE`
- `REJECTED`
- `TIMED_OUT`

POL implication:
- ECLSS is commandable, but not through the same handshake model as Airlock
- use the writeback object as the authoritative progression marker
- do not treat accepted setpoints as immediate physical success

## 6.3 Sabatier PEA

PEA ID:
- `SABATIER-PEA-001`

Purpose:
- methanation
- feed conditioning
- hydrogen recovery
- catalyst regeneration
- contingency actions including emergency vent and safe shutdown

Current services:
- `Methanation`
- `FeedConditioning`
- `HydrogenRecovery`
- `CatalystRegeneration`
- `SabatierContingency`

Current command set:
- `START_METHANATION`
- `STOP_METHANATION`
- `SET_CO2_FEED_VALVE_PCT`
- `SET_H2_FEED_VALVE_PCT`
- `SET_REACTOR_TEMP_TARGET_C`
- `SET_REACTOR_PRESSURE_TARGET_BAR`
- `START_FEED_CONDITIONING`
- `STOP_FEED_CONDITIONING`
- `START_HYDROGEN_RECOVERY`
- `STOP_HYDROGEN_RECOVERY`
- `START_CATALYST_REGEN`
- `STOP_CATALYST_REGEN`
- `EXECUTE_EMERGENCY_VENT`
- `EXECUTE_SAFE_SHUTDOWN`

Important process values:
- `reactor_temp_c`
- `reactor_temp_target_c`
- `reactor_pressure_bar`
- `reactor_pressure_target_bar`
- `co2_feed_kgph`
- `co2_feed_valve_pct`
- `h2_feed_kgph`
- `h2_feed_valve_pct`
- `conversion_efficiency_pct`
- `methane_production_kgph`
- `water_production_kgph`
- `catalyst_health_pct`
- `power_kw`
- `methanation_enabled`
- `feed_conditioning_enabled`
- `hydrogen_recovery_enabled`
- `catalyst_regen_active`
- `emergency_vent_active`
- alarm flag for reactor temperature

Important coupling to other PEAs:
- Sabatier feed behavior is currently coupled to ECLSS CO2 capture availability

POL implication:
- Sabatier is operational, but it should be treated as an asynchronous subsystem with staged writeback
- a reactor setpoint write is not a terminal success signal
- sequence completion should follow final writeback state, not just command acceptance

## 7. Current API Surfaces The POL Should Prefer

### For inventory and runtime state
- `GET /api/v1/pea`
- `GET /api/v1/pea/{pea_id}`
- `GET /api/v1/pea/{pea_id}/opcua`
- `GET /api/v1/pea/{pea_id}/mtp/tree`

### For ECLSS and Sabatier operator state and writeback
- `GET /api/v1/pea/{pea_id}/operator-state`
- `POST /api/v1/pea/{pea_id}/operator-state`
- `POST /api/v1/pea/{pea_id}/advanced-command`

### For Airlock command interaction
- `POST /api/commands/operator/write`
- `POST /api/commands/remote/write`
- `POST /api/v1/pea/{pea_id}/services/{service_tag}/command`

### For topology and i3X discovery
- `GET /api/v1/namespaces`
- `GET /api/v1/objecttypes`
- `GET /api/v1/objects`
- `GET /api/v1/i3x/pea`
- `GET /api/v1/i3x/pea/{pea_id}`

## 8. Known Current Limitations The POL Should Expect

- Airlock is more complete than ECLSS and Sabatier in MTP handshake behavior.
- ECLSS and Sabatier are modeled as commandable subsystems, but still not full Airlock-style request/response PEAs.
- i3X write path is not implemented for actual control.
- future PEAs are modeled in the catalog but are not fully simulated.
- command source `pol` is currently treated as operator-side authority for subsystem commands.
- only one active subsystem writeback is realistically expected per active subsystem at a time.

## 9. Proposed WinCC OA DPT Model

These DPTs are proposed from the current simulator surface. They are not already implemented in WinCC OA; they are the recommended DPT families to create so the POL can map cleanly onto the current runtime.

## 9.1 Core DPTs

### `UH_PEARef`
Use one instance per PEA.

Fields:
- `peaId : string`
- `peaType : string`
- `name : string`
- `tierId : string`
- `namespaceUri : string`
- `rootPath : string`
- `opcuaEndpoint : string`
- `serviceTagPrimary : string`
- `implementationStatus : string`

### `UH_PEARuntime`
Use one instance per active PEA.

Fields:
- `deployed : bool`
- `running : bool`
- `healthState : string`
- `updatedAtMs : ulong`
- `lastTransitionMs : ulong`
- `activeCommandRunning : bool`

### `UH_PEACommandability`
Use one instance per commandable PEA.

Fields:
- `operationMode : string`
- `sourceMode : string`
- `commandEn : bool`
- `commandEnReason : string`
- `operatorControlEnabled : bool`
- `remoteControlEnabled : bool`
- `dispatchContract : string`

### `UH_PEAWriteback`
Use one instance for `ECLSS-PEA-001` and one for `SABATIER-PEA-001`.

Fields:
- `sequenceId : uint`
- `source : string`
- `command : string`
- `param1 : float`
- `param2 : float`
- `targetProcedure : string`
- `stage : string`
- `message : string`
- `rejectReason : string`
- `issuedAtMs : ulong`
- `applyAfterMs : ulong`
- `appliedAtMs : ulong`
- `settleDeadlineMs : ulong`
- `completedAtMs : ulong`

Recommended instance split:
- `<PEA>.WritebackActive`
- `<PEA>.WritebackLastCompleted`

### `UH_PEAServiceState`
Use one instance per service per active PEA.

Fields:
- `serviceName : string`
- `state : string`
- `stateCode : uint`
- `activeProcedure : string`
- `transitionActive : bool`
- `commandEn : bool`

## 9.2 Airlock-Specific DPTs

### `UH_AirlockProcess`
Fields:
- `pressurePa : float`
- `temperatureK : float`
- `o2Percent : float`
- `innerDoorPositionPct : float`
- `outerDoorPositionPct : float`
- `innerLockEngaged : bool`
- `outerLockEngaged : bool`
- `equalizeValvePct : float`
- `ventValvePct : float`
- `pumpOn : bool`
- `pumpCurrentA : float`
- `stateName : string`

### `UH_AirlockAlarm`
Fields:
- `highPressureAlarmActive : bool`
- `lowPressureAlarmActive : bool`
- `interlockViolation : bool`
- `leakDetected : bool`
- `outOfSpec : bool`
- `alarmSummary : string`

### `UH_AirlockCommandReq`
Recommended one DPT for operator and one for remote instances.

Fields:
- `sequenceId : uint`
- `command : string`
- `param1 : float`
- `param2 : float`
- `execute : bool`

### `UH_AirlockCommandRsp`
Recommended one DPT for operator and one for remote instances.

Fields:
- `ackSequenceId : uint`
- `status : string`
- `rejectReason : string`
- `lastUpdateTimeMs : ulong`

### `UH_AirlockActiveCommand`
Fields:
- `command : string`
- `source : string`
- `sequenceId : uint`
- `param1 : float`
- `param2 : float`
- `state : string`
- `progressPct : float`
- `blockingCondition : string`
- `startTimeMs : ulong`
- `lastUpdateTimeMs : ulong`

## 9.3 ECLSS-Specific DPTs

### `UH_ECLSSProcess`
Fields:
- `cabinPressureKpa : float`
- `cabinPressureTargetKpa : float`
- `o2Percent : float`
- `co2Ppm : float`
- `humidityPct : float`
- `humidityTargetPct : float`
- `waterRecoveryPct : float`
- `waterRecoveryTargetPct : float`
- `co2CaptureKgph : float`
- `co2FlowSetpointKgph : float`
- `o2GenerationKgph : float`
- `o2GenerationSetpointKgph : float`
- `powerKw : float`
- `co2ScrubberRunning : bool`
- `electrolyzerRunning : bool`
- `waterProcessorRunning : bool`
- `branchIsolated : bool`
- `safeHavenEnabled : bool`
- `alarmHighCo2 : bool`
- `alarmLowO2 : bool`

### `UH_ECLSSOperatorState`
Fields:
- `operationMode : string`
- `sourceMode : string`
- `commandEn : bool`
- `commandEnReason : string`
- `operatorControlEnabled : bool`
- `remoteControlEnabled : bool`
- `activeProcedure : string`
- `lastCommand : string`
- `lastCommandStatus : string`
- `lastRejectReason : string`

### `UH_ECLSSCommand`
Fields:
- `sequenceId : uint`
- `source : string`
- `command : string`
- `param1 : float`
- `param2 : float`

## 9.4 Sabatier-Specific DPTs

### `UH_SabatierProcess`
Fields:
- `reactorTempC : float`
- `reactorTempTargetC : float`
- `reactorPressureBar : float`
- `reactorPressureTargetBar : float`
- `co2FeedKgph : float`
- `co2FeedValvePct : float`
- `h2FeedKgph : float`
- `h2FeedValvePct : float`
- `conversionEfficiencyPct : float`
- `methaneProductionKgph : float`
- `waterProductionKgph : float`
- `catalystHealthPct : float`
- `powerKw : float`
- `methanationEnabled : bool`
- `feedConditioningEnabled : bool`
- `hydrogenRecoveryEnabled : bool`
- `catalystRegenActive : bool`
- `emergencyVentActive : bool`
- `alarmReactorTemp : bool`

### `UH_SabatierOperatorState`
Fields:
- `operationMode : string`
- `sourceMode : string`
- `commandEn : bool`
- `commandEnReason : string`
- `operatorControlEnabled : bool`
- `remoteControlEnabled : bool`
- `activeProcedure : string`
- `lastCommand : string`
- `lastCommandStatus : string`
- `lastRejectReason : string`

### `UH_SabatierCommand`
Fields:
- `sequenceId : uint`
- `source : string`
- `command : string`
- `param1 : float`
- `param2 : float`

## 9.5 Diagnostics DPT

### `UH_PEADiagnostics`
Recommended for all active PEAs where data exists.

Fields:
- `endpointUrl : string`
- `activeSecurityMode : string`
- `securityModesEnabled : dyn_string`
- `serverStartTimeMs : ulong`
- `serverUptimeSec : float`
- `connectedClientCount : uint`
- `connectedClientSummary : string`
- `subscriptionCount : uint`
- `publishingRateHz : float`
- `lastRejectedCommand : string`
- `lastError : string`

## 10. Recommended WinCC OA Instance Naming

A practical naming pattern would be:
- `Underhill.AIRLOCK-PEA-001.Ref`
- `Underhill.AIRLOCK-PEA-001.Runtime`
- `Underhill.AIRLOCK-PEA-001.Process`
- `Underhill.AIRLOCK-PEA-001.Alarm`
- `Underhill.AIRLOCK-PEA-001.OperatorReq`
- `Underhill.AIRLOCK-PEA-001.OperatorRsp`
- `Underhill.AIRLOCK-PEA-001.RemoteReq`
- `Underhill.AIRLOCK-PEA-001.RemoteRsp`
- `Underhill.AIRLOCK-PEA-001.ActiveCommand`
- `Underhill.ECLSS-PEA-001.Ref`
- `Underhill.ECLSS-PEA-001.Runtime`
- `Underhill.ECLSS-PEA-001.Process`
- `Underhill.ECLSS-PEA-001.OperatorState`
- `Underhill.ECLSS-PEA-001.WritebackActive`
- `Underhill.ECLSS-PEA-001.WritebackLastCompleted`
- `Underhill.SABATIER-PEA-001.Ref`
- `Underhill.SABATIER-PEA-001.Runtime`
- `Underhill.SABATIER-PEA-001.Process`
- `Underhill.SABATIER-PEA-001.OperatorState`
- `Underhill.SABATIER-PEA-001.WritebackActive`
- `Underhill.SABATIER-PEA-001.WritebackLastCompleted`

## 11. Bottom Line For The POL

If a WinCC OA POL is controlling Underhill today, it should assume:
- Airlock is the reference implementation for rich command semantics.
- ECLSS and Sabatier are operational but should be treated as asynchronous staged-writeback subsystems.
- only three PEAs are truly active right now.
- the rest of the Mars Base is present as a base model and engineering roadmap, not as deployable runtime equipment.
- the safest current orchestration strategy is descriptor read, commandability check, one-command issue, writeback tracking, then mission progression.
