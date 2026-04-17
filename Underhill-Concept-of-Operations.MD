# Underhill Concept of Operations

**CANONICAL REFERENCE DOCUMENT** — Operational and engineering reference for the Underhill Mars habitat base. Defines all subsystem architectures, control procedures, and mission-critical interlocks.

Combines MTP (Modular Type Package) control architecture with NASA operational standards, detailing how Underhill's four integrated subsystems (Airlock, ECLSS, Sabatier, and future systems) sustain crew life, manage atmosphere/water, produce propellant, and provide advanced gas processing for the Mars surface campaign.

## Purpose & Scope

This document defines:
1. **MTP normative model** for Underhill subsystems using PascalCase naming
2. **Operational context** for each subsystem grounded in NASA standards/NTRS research
3. **Implementation status** (IMPLEMENTED, TO_IMPLEMENT) for all PEAs
4. **Control module binding pattern** ensuring every procedure connects to equipment

Terminology:
- `Service` = operational capability/mission phase (example: `Depressurizing`)
- `Procedure` = control action bound to exactly one control module (example: `OpeningPressureReleaseValve`)
- `Parameter` = typed binary or analog value on a procedure/module (example: `OpenPercentage`)
- All names use **PascalCase**.

## Canonical MTP Definitions

### 1. Pea (Physical Equipment Assembly)
A subsystem container representing a distinct operational capability. Underhill PEAs organized by criticality tier:

**Tier 1: Core Life Support & Habitat**
- **Airlock** (IMPLEMENTED) — crew ingress/egress, atmosphere reclaim, hatch control
- **ECLSS** (TO_IMPLEMENT) — atmosphere revitalization, oxygen generation, water recovery, thermal control
- **HabitatStructure** (TO_IMPLEMENT) — pressure envelope, structural integrity, segment isolation, leak containment
- **Water** (TO_IMPLEMENT) — ice extraction, water treatment, potable distribution, wastewater handling

**Tier 2: Power, Thermal, & ISRU**
- **PowerGen** (TO_IMPLEMENT) — solar arrays, nuclear reactor, backup power generation for Mars operations
- **PowerGrid** (TO_IMPLEMENT) — power distribution, load shedding, battery dispatch, grid stability control
- **ThermalControl** (TO_IMPLEMENT) — advanced heating/cooling for dome and life-support loops, radiator management
- **Sabatier** (TO_IMPLEMENT) — CO2 methanation, hydrogen recovery, Mars ISRU coupling

**Tier 3: Production, Safety, & Advanced Operations**
- **Farm** (TO_IMPLEMENT) — greenhouse production, aquaculture, food synthesis, CO2 reclamation
- **Safety** (TO_IMPLEMENT) — emergency coordination, hazard monitoring, incident response, evacuation control
- **NobleGasRecovery** (TO_IMPLEMENT) — argon/neon/krypton extraction and purification
- **IsotopeFractionation** (TO_IMPLEMENT) — oxygen-18/deuterium separation for science and ISRU
- **AdvancedAtmospherePolishing** (TO_IMPLEMENT) — catalytic contaminant removal beyond ECLSS baseline
- **GasStorageAndDistribution** (TO_IMPLEMENT) — specialty-gas storage hub and precision metering

### 2. Service
A mission-meaningful operational mode of a PEA. Examples:
- `Depressurizing` (Airlock service)
- `OxygenGeneration` (ECLSS service)
- `Methanation` (Sabatier service)

A Service owns:
- Its own `ServiceStateMachine` (current state, transitions, timing)
- A `ProcedureSet` (all procedures executable within this service)
- Service-level interlocks and safety constraints
- Mode/source control channels (Operator, Remote, System)

### 3. Procedure
A control action bound to **exactly one** primary control module. Examples:
- `OpeningPressureReleaseValve` (in Airlock `Depressurizing` service)
- `StartingVacuumPump` (in Airlock `Depressurizing` service)
- `SettingCo2RemovalFlowRate` (in ECLSS `Co2Removal` service)

A Procedure contains:
- Binary parameters (commands: `OpenCommand`, `StartCommand`, etc.)
- Analog parameters (setpoints: `OpenPercentage`, `TargetPressureKpa`, etc.)
- Reports (feedback: `ActualOpenPercentage`, `LastExecutionResult`, etc.)

### 4. ControlModule
The equipment endpoint that a Procedure drives. Constrained types:
- `BinVlv` — binary valve (open/close only)
- `AnaVlv` — analog valve (variable position, 0-100%)
- `BinDrv` — binary drive (on/off, lock/unlock)
- `AnaDrv` — analog drive (variable speed, power, position)

Example: Procedure `OpeningPressureReleaseValve` drives ControlModule `PressureReleaseValve` of type `AnaVlv`.

### 5. Parameter
A typed value linked to a Procedure (and thus to the control module). Two kinds:
- **BinaryParameters**: `OpenCommand`, `StartCommand`, `LockCommand`, `StopCommand` (boolean semantics)
- **AnalogParameters**: `OpenPercentage`, `TargetPressureKpa`, `FlowRateKgPerHour` (numeric range with units)

## Naming Rules (PascalCase Standard)

- **Services**: `Depressurizing`, `Co2Removal`, `WaterRecovery`, `Methanation`
- **Procedures**: `OpeningPressureReleaseValve`, `StartingVacuumPump`, `SettingDepressurizationRate`, `LockingInnerHatch`
- **ControlModules**: `PressureReleaseValve`, `VacuumPumpDrive`, `InnerHatchDoorDrive`, `Co2ScrubberBedADrive`
- **AnalogParameters**: `OpenPercentage`, `TargetPressureKpa`, `FlowRateKgPerHour`, `TargetTemperatureC`
- **BinaryParameters**: `OpenCommand`, `CloseCommand`, `StartCommand`, `StopCommand`, `LockCommand`, `UnlockCommand`, `EnableCommand`
- **Reports**: `ActualOpenPercentage`, `CurrentPressureKpa`, `LastExecutionResult`, `IsRunning`, `CurrentA`
- **Command objects**: `OperatorCommands`, `RemoteCommands`, `ActiveCommand`, `LastRejectReason`

---

# PEA Models & Implementation Status

---

## AIRLOCK PEA (IMPLEMENTED)

**Status**: Code complete in `backend/src/sim.rs`, `backend/src/opcua.rs`, `backend/src/main.rs` with 7 services and procedures mapped to control modules.

**Operational Context** (NASA-backed requirements):
- **Purpose**: crew ingress/egress state machine, atmosphere reclaim, dust/decontamination, emergency repress
- **Mars EVA constraints**: pumpdown/repressurization cycles, high EVA cadence (future ~24 hrs/week), dust ingestion risk, crew decompression sickness avoidance
- **Key interlocks**: no concurrent inner/outer hatch open, no depress if hatches not secured, no repress if pressure gradients exceed structural limits, force isolation on leak/fire detection
- **Cross-system dependency**: cabin pressure band (34.5–103.4 kPa), controlled pressure ramps for crew safety, O2 fraction compatibility with final habitat pressure

### Simulation Model
The airlock has a fully implemented deterministic simulation in `backend/src/sim.rs` driving its service/state state machine. Each service transitions through states (Idle, Executing, Completed) with procedure progress updated on a 200 ms tick. Physical actions (valve travel, pump spin‑up) are modeled with first‑order lags (2–5 s) and pressure dynamics solved via simple volume/flow algebra. Fault‑injection hooks allow hatch jams, pump failures and chamber leaks while `validate_request()` enforces interlocks.

### Service Set
1. `Depressurizing` — pumpdown via vacuum pump and pressure relief valve
2. `Pressurizing` — repressurization via supply valve
3. `HatchTransfer` — hatch motion and locking
4. `AtmosphereReclaim` — pre-vent gas recovery to reduce consumables loss
5. `EmergencyRepressurizing` — rapid repress for aborts/contingency
6. `Isolation` — airlock/habitat boundary sealing, fault reset

### Procedure Model By Service

#### `Depressurizing`
- **`OpeningPressureReleaseValve`**
  - ControlModule: `PressureReleaseValve` (AnaVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - AnalogParameters: `OpenPercentage`
  - Reports: `ActualOpenPercentage`, `PressureDropRateKpaPerSec`

- **`StartingVacuumPump`**
  - ControlModule: `VacuumPumpDrive` (AnaDrv)
  - BinaryParameters: `StartCommand`, `StopCommand`
  - AnalogParameters: `PumpSpeedPercentage`
  - Reports: `IsRunning`, `CurrentA`, `ActualPumpSpeedPercentage`

- **`SettingDepressurizationRate`**
  - ControlModule: `VacuumPumpDrive` (AnaDrv)
  - AnalogParameters: `PumpSpeedPercentage`, `TargetPressureKpa`, `MaxDepressRateKpaPerSec`
  - Reports: `CurrentPressureKpa`, `ProgressToTargetPercentage`

#### `Pressurizing`
- **`OpeningPressurizationSupplyValve`**
  - ControlModule: `PressurizationSupplyValve` (AnaVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - AnalogParameters: `OpenPercentage`
  - Reports: `ActualOpenPercentage`, `PressureRiseRateKpaPerSec`

- **`SettingPressurizationRate`**
  - ControlModule: `PressurizationSupplyValve` (AnaVlv)
  - AnalogParameters: `OpenPercentage`, `TargetPressureKpa`, `MaxPressurizationRateKpaPerSec`
  - Reports: `CurrentPressureKpa`, `ProgressToTargetPercentage`

#### `HatchTransfer`
- **`UnlockingInnerHatch`**
  - ControlModule: `InnerHatchLockDrive` (BinDrv)
  - BinaryParameters: `UnlockCommand`, `LockCommand`
  - Reports: `IsLocked`

- **`LockingInnerHatch`**
  - ControlModule: `InnerHatchLockDrive` (BinDrv)
  - BinaryParameters: `LockCommand`
  - Reports: `IsLocked`

- **`SettingInnerHatchPosition`**
  - ControlModule: `InnerHatchDoorDrive` (AnaDrv)
  - AnalogParameters: `OpenPercentage`, `MaxOpenSpeedPctPerSec`
  - Reports: `ActualOpenPercentage`, `MotorCurrentA`

- **`UnlockingOuterHatch`**
  - ControlModule: `OuterHatchLockDrive` (BinDrv)
  - BinaryParameters: `UnlockCommand`, `LockCommand`
  - Reports: `IsLocked`  
    > _Note_: the report name reflects the underlying lock state (true when the
    lock is engaged, false when the hatch is unlocked).  The same `IsLocked`
    report is used for both locking and unlocking procedures.


- **`LockingOuterHatch`**
  - ControlModule: `OuterHatchLockDrive` (BinDrv)
  - BinaryParameters: `LockCommand`
  - Reports: `IsLocked`

- **`SettingOuterHatchPosition`**
  - ControlModule: `OuterHatchDoorDrive` (AnaDrv)
  - AnalogParameters: `OpenPercentage`, `MaxOpenSpeedPctPerSec`
  - Reports: `ActualOpenPercentage`, `MotorCurrentA`

#### `AtmosphereReclaim`
- **`OpeningReclaimValve`**
  - ControlModule: `ReclaimValve` (AnaVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - AnalogParameters: `OpenPercentage`
  - Reports: `ActualOpenPercentage`, `ReclaimedGasFlowKgPerHour`

- **`SettingReclaimFlowRate`**
  - ControlModule: `ReclaimValve` (AnaVlv)
  - AnalogParameters: `OpenPercentage`, `RecoveryTargetPercentage`, `MaxReclaimFlowKgPerHour`
  - Reports: `ReclaimedQuantityKg`, `ReclaimEfficiencyPercentage`

#### `EmergencyRepressurizing`
- **`OpeningEmergencyRepressValve`**
  - ControlModule: `EmergencyRepressValve` (AnaVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - AnalogParameters: `OpenPercentage`
  - Reports: `ActualOpenPercentage`, `PressureRiseRateKpaPerSec`

- **`SettingEmergencyRepressRate`**
  - ControlModule: `EmergencyRepressValve` (AnaVlv)
  - AnalogParameters: `OpenPercentage`, `TargetPressureKpa`, `MaxRepressRateKpaPerSec`
  - Reports: `CurrentPressureKpa`

#### `Isolation`
- **`ClosingIsolationBoundaryValves`**
  - ControlModule: `IsolationBoundaryValve` (BinVlv)
  - BinaryParameters: `CloseCommand`, `OpenCommand`
  - Reports: `IsClosed`

- **`LockingAllHatches`**
  - ControlModule: `HatchInterlockDrive` (BinDrv)
  - BinaryParameters: `LockCommand`
  - Reports: `AllHatchesLocked`

- **`AbortingCurrentService`**
  - ControlModule: `AbortRelayDrive` (BinDrv)
  - BinaryParameters: `AbortCommand`
  - Reports: `AbortInitiated`, `AbortStatus`

- **`ResettingFaultState`**
  - ControlModule: `FaultResetRelayDrive` (BinDrv)
  - BinaryParameters: `ResetCommand`
  - Reports: `FaultCleared`, `SystemHealthy`

### Airlock-Specific Interlocks
- No outer hatch open if inner hatch open (hard interlock)
- No depress if inner hatch not confirmed closed/locked
- No repress if structural dP limit exceeded
- No mode switch mid-critical transition unless emergency override
- Force isolation on leak/fire detection

---

## ECLSS PEA (TO_IMPLEMENT)

**Status**: Specification phase. Requires implementation of 5 services with procedures for atmosphere revitalization, O2 generation, water recovery, thermal control, and contingency.

**Operational Context** (NASA STD-3001 & ECLSS references):
- **Purpose**: closed-loop atmosphere/water/oxygen management, crew survival, consumables minimization
- **Cabin atmosphere targets**: ppO2 18.0–23.8 kPa, ppCO2 ≤0.53 kPa (mission-average), temp 18.3–26.7°C, RH 25–75%
- **High EVA cadence impact**: frequent pressure cycling, high CO2 load from post-EVA crew, water/contaminant ingress from dust
- **Integration constraints**: must operate under variable power, support degraded-mode operations, maintain loop closure under power loss
- **Redundancy model**: branch isolation/restart sequencing, automated failover on component failure
- **Source**: NASA-STD-3001 Appendix E Vol 2, NASA ECLSS reference page, NASA NTRS CO2 control/contaminant literature

### Simulation Model
ECLSS simulation will be state‑machine driven with each service (CO2 removal, oxygen gen, etc.) implemented as a finite‑state automaton. A discrete‑time solver (200 ms step) updates mass balances for CO2, O2 and water loops using linear ODE approximations; scrubber bed capacity, electrolyzer output and filter saturation are tracked. Interlocks and degraded modes are checked each tick.

### Service Set
1. `Co2Removal` — CO2 capture (CDRA-like or equivalent), trace contaminant cleanup
2. `OxygenGeneration` — electrolysis O2 production, O2 distribution/compression
3. `WaterRecovery` — wastewater processing, reclamation, output quality control
4. `AtmosphereConditioning` — thermal management, humidity control, circulation
5. `ContingencyLifeSupport` — fire/smoke response, degraded-mode ECLSS, safe-haven profiles

### Procedure Examples (To Be Fully Specified)
#### `Co2Removal`
- `StartingCo2ScrubberBedA` (BinDrv: `Co2ScrubberBedADrive`) — activate first/primary CO2 bed
- `SettingCo2RemovalFlowRate` (AnaDrv: `Co2FlowDrive`) — adjust removal capacity
- `RegeneratingCo2ScrubberBed` (AnaDrv: `Co2RegenerationDrive`) — thermal/pressure swing regeneration
- `PurgingTraceContaminants` (AnaDrv: `ContaminantPurgeDrive`) — remove H2S, NH3, volatiles

#### `OxygenGeneration`
- `StartingElectrolyzerStack` (BinDrv: `ElectrolyzerStackDrive`) — electrolysis initiation
- `SettingOxygenGenerationRate` (AnaDrv: `OxygenGenerationDrive`) — control production to demand
- `CompressingOxygenProduct` (AnaDrv: `O2CompressorDrive`) — pressure boost for storage/distribution

#### `WaterRecovery`
- `StartingWaterProcessor` (BinDrv: `WaterProcessorDrive`) — activate wastewater path
- `RegeneratingWaterFilters` (AnaDrv: `WaterFilterRegenerationDrive`) — filter lifecycle management
- `SettingWaterPurityTarget` (AnaDrv: `WaterQualityDrive`) — output spec enforcement

#### `AtmosphereConditioning`
- `SettingCabinTemperature` (AnaDrv: `CabinThermalDrive`) — thermal setpoint control
- `SettingCabinHumidity` (AnaDrv: `CabinHumidityDrive`) — RH regulation

#### `ContingencyLifeSupport`
- `IsolatingEclssBranch` (BinVlv: `EclssIsolationValve`) — segment fault containment
- `EnablingSafeHavenProfile` (BinDrv: `SafeHavenDrive`) — minimal O2/CO2 safe mode

### ECLSS Interlocks (To Be Enforced)
- No CRA hydrogen feed without proper downstream handling enabled
- No O2-increase command if over-oxygen hazard threshold predicted
- Automatic degraded mode on power loss
- Automatic safe-haven profile activation on fire/smoke hazard

---

## SABATIER PEA (TO_IMPLEMENT)

**Status**: Specification phase. Requires implementation of 5 services and 11+ procedures for CO2 methanation, hydrogen recovery, Mars ISRU integration, and safe shutdown logic.

**Operational Context** (NASA MOXIE, Sabatier thermodynamics, Mars ISRU):
- **Purpose**: Mars surface ISRU methane/oxygen production, hydrogen recovery, ECLSS integration
- **Chemistry**: CO2 + H2 → CH4 + H2O, reactor control (250–450°C, 1–10 bar estimated), stoichiometric ratio ~H2:CO2 4:1
- **Mars coupling**: high-CO2 low-pressure atmosphere intake, integration with oxygen-recovery paths, water-purification feedback
- **Catalyst management**: thermal regeneration of Ni or other catalysts, coking risk monitoring, deactivation avoidance
- **Power awareness**: Sabatier is energy-intensive; design load-following, grid-tied autonomy modes
- **Sources**: NASA MOXIE mission completion, NTRS methane post-processor development, CO2 methanation thermodynamic literature, LPI-002142 lunar ISRU automation patterns

### Service Set
1. `Methanation` — main Sabatier conversion, feed valve control, ratio management
2. `FeedConditioning` — water purification, electrolyzer pressurization, feed quality
3. `HydrogenRecovery` — post-processor, product separation, recycle compressor
4. `CatalystRegeneration` — thermal/pressure swing cycles, health monitoring
5. `SabatierContingency` — safe shutdown, emergency vent/isolation, off-nominal chemistry response

### Procedure Examples (To Be Fully Specified)
#### `Methanation`
- `OpeningCo2FeedValve` (AnaVlv: `Co2FeedValve`) — CO2 supply control
- `OpeningH2FeedValve` (AnaVlv: `H2FeedValve`) — hydrogen supply control
- `SettingH2Co2MolarRatio` (AnaDrv: `FeedRatioDrive`) — stoichiometric ratio maintenance
- `SettingReactorTemperature` (AnaDrv: `ReactorTemperatureDrive`) — warmup & steady-state control (*Inferred setpoint: 250–450°C*)
- `SettingReactorPressure` (AnaDrv: `ReactorPressureDrive`) — pressure regulation (*Inferred setpoint: 1–10 bar*)

#### `FeedConditioning`
- `StartingWaterPurificationTrain` (BinDrv: `WaterPurificationDrive`) — electrolyzer feed path
- `SettingFeedWaterPurity` (AnaDrv: `FeedWaterQualityDrive`) — quality enforcement (*Inferred target: <50 ppm contaminants*)
- `PressurizingElectrolyzerOutput` (AnaDrv: `ElectrolyzerPressureDrive`) — H2 supply pressure

#### `HydrogenRecovery`
- `StartingMethanePostProcessor` (BinDrv: `MethanePostProcessorDrive`) — product separation activation
- `SettingHydrogenRecoveryTarget` (AnaDrv: `HydrogenRecoveryDrive`) — recycle fraction control (*Inferred target: 85–95% recovery*)
- `CompressingRecycleStreams` (AnaDrv: `RecycleCompressorDrive`) — loop closure pressure boost

#### `CatalystRegeneration`
- `StartingCatalystRegenerationCycle` (AnaDrv: `CatalystRegenerationDrive`) — thermal/pressure swing activation
  - BinaryParameters: `StartCommand`, `AbortCommand`
  - AnalogParameters: `RegenerationTemperatureC`, `RegenerationDurationMinutes`
  - Reports: `CatalystHealthIndex`, `CokingRiskLevel`

#### `SabatierContingency`
- `ExecutingSafeShutdown` (BinDrv: `SabatierSafetyDrive`) — orderly depressurization & cool-down
- `ExecutingEmergencyVent` (BinVlv: `EmergencyVentValve`) — rapid pressure relief

### Sabatier Interlocks (To Be Enforced)
- Block `START_METHANATION` unless warmup complete and purge complete
- Block feed increase if temperature runaway margin violated (*Inferred: e.g., maintain >50°C margin to runaway*)
- Auto-shutdown on pressure excursion or impurity threshold exceedance
- Force safe vent/isolation on leak detection

---

## POWERGEN PEA (TO_IMPLEMENT)

**Status**: Critical infrastructure. Specification phase. Solar and nuclear power generation for Underhill base operations across 18–24 month surface campaign with Martian day/night and seasonal variations.

**Operational Context** (Mars power environment):
- **Purpose**: primary power generation for all Underhill subsystems (~1–2 MW baseline load, peaks to 5+ MW during active operations)
- **Challenge**: Mars solar irradiance ~43% Earth's (430 W/m² vs 1000 W/m² at Earth); high dust deposition (need frequent cleansing); long polar winter (2–3 months reduced solar); seasonal CO2 frost buildup
- **Dual-path strategy**: solar array farm (primary seasonal source) + nuclear reactor (baseload during winter); battery + supercapacitor energy storage; diesel/fuel-cell backup for critical loads
- **Integration**: supplies power to every subsystem with priority tiering (life support → production → science)

### Service Set
1. `SolarGeneration` — daytime power production, array management, dust mitigation
2. `NuclearGeneration` — baseload reactor operation, thermal management, fuel cycling
3. `EnergyStorage` — battery dispatch, state-of-charge management, emergency reserves
4. `BackupPowerGeneration` — fuel-cell or diesel startup, emergency power routing

### Procedure Examples
#### `SolarGeneration`
- **`StartingSolarArrayField`** (BinDrv: `SolarArrayDrive`) — activate string controls, bypass parasitic load
- **`SettingSolarOutputSetpoint`** (AnaDrv: `SolarDispatchDrive`) — MPP tracking, power regulation to grid demand
- **`CleaningSolarPanelsFromDust`** (BinDrv: `PanelCleaningDrive`) — mechanical wipers or thermal cycling for dust removal

#### `NuclearGeneration`
- **`StartingReactorCore`** (BinDrv: `ReactorStartupDrive`) — control rod insertion, coolant circulation startup
- **`SettingReactorThermalPower`** (AnaDrv: `ReactionRateDrive`) — fission rate control, power ramp
- **`SettingCoolantFlowRate`** (AnaDrv: `CoolantCirculationDrive`) — thermal balance, rejection capacity

#### `EnergyStorage`
- **`StartingBatteryDischarge`** (AnaDrv: `BatteryDispatchDrive`) — SOC management, peak shaving, voltage regulation
- **`StartingSupercapacitorCharging`** (BinDrv: `CapacitorChargeDrive`) — fast-transient load handling

#### `BackupPowerGeneration`
- **`StartingFuelCell`** (BinDrv: `FuelCellStartupDrive`) — hydrogen feed, oxygen feed, electrical output commissioning
- **`AutomaticallyActivatingBackupOnLossOfPrimaryPower`** (BinDrv: `BackupActivationRelay`) — UPS function

### PowerGen Interlocks (To Be Enforced)
- No solar MPP tracking if array temperature >100°C (protection against thermal damage)
- Automatic battery discharge if solar production drops >500 kW/min (cloud transient or dust storm)
- Nuclear fission rate limits enforced if coolant temperature approaches 150°C
- Automatic fuel-cell startup if battery SOC drops below 20%
- Reactor scram (emergency shutdown) on coolant loss or overpressure >15 bar

### Simulation Model
PowerGen simulation combines a solar irradiance/time-of-day model with dust‑accumulation decay and nuclear reactor startup lags. A discrete energy balance updates battery SOC and supercapacitor state each tick; backup generators are state‑machine triggered. Output feeds PowerGrid simulation with priority logic.

---

## POWERGRID PEA (TO_IMPLEMENT)

**Status**: Critical infrastructure. Specification phase. Electrical distribution, load management, and grid stability control for Underhill's multi-MW power system.

**Operational Context** (Mars grid operations):
- **Purpose**: distribute generated power across life support, production, science, and contingency loads; maximize reliability despite source intermittency; enable graceful degradation on subsystem malfunction
- **Topology**: ring-bus distribution with automated feeder reconfiguration; sectionalizing breakers for fault isolation
- **Load tiers**: Tier 1 (mandatory life support: Airlock, ECLSS core), Tier 2 (mission-critical: Sabatier, Farm, ECLSS auxiliary), Tier 3 (production: Mining, Materials), Tier 4 (science/optional)
- **Strategy**: predictable load shedding on power deficit; priority-based restoration after supply return; real-time battery State-of-Charge and thermal margin monitoring

### Service Set
1. `HVDistribution` — voltage regulation, feeder switching, breaker control
2. `LoadShedding` — priority-based demand response, controlled outage sequencing
3. `StorageDispatch` — battery/capacitor coordination, peak shaving, UPS functions
4. `GridStabilityControl` — frequency regulation, voltage support, transient response

### Procedure Examples
#### `HVDistribution`
- **`ConnectingFeeder`** (BinVlv: `FeederBreakerSolenoid`) — close breaker, enable power feed
- **`IsolatingFaultedCircuit`** (BinVlv: `SectionalizingBreaker`) — detect overcurrent, trip breaker to isolate zone
- **`SettingBusVoltageSetpoint`** (AnaDrv: `VoltageRegulatorDrive`) — maintain 400V ±5% across load variations

#### `LoadShedding`
- **`SheddingTier3ProductionLoads`** (BinDrv: `Mining_ShedRelay`) — disconnect non-critical industrial equipment
- **`SheddingTier2MissionSecondaryServices`** (BinDrv: `Farm_ShedRelay`) — reduce agriculture to essential species only
- **`SettingLoadShedSequence`** (AnaDrv: `LoadShedPriorityDrive`) — rank loads by criticality, enforce time delays between successive sheds

#### `StorageDispatch`
- **`StartingBatteryDischargeToMeetPeakLoad`** (AnaDrv: `BatteryDispatchDrive`) — synchronized discharge profile, SOC tracking
- **`ChargingBatteryFromSolarSurplus`** (AnaDrv: `ChargeControlDrive`) — absorb midday peak solar, charge-rate management

#### `GridStabilityControl`
- **`RegulatingGridFrequency`** (AnaDrv: `FrequencyControlDrive`) — load-following, ramp-rate limiting on generation changes
- **`ActivatingVoltageSupport`** (BinDrv: `VarCompensationRelay`) — capacitor bank or reactive power injection on voltage sags

### PowerGrid Interlocks (To Be Enforced)
- No load restoration until 2+ power sources confirmed stable >60 seconds
- Automatic feeder isolation on sustained overcurrent (>120% rated 5 seconds)
- Tier 4 science loads automatically disconnected if any Tier 1 load at risk
- No new production loads activated if reserve margin <15% of current demand
- Automatic islanding mode if main grid frequency drifts >±5% nominal

### Simulation Model
PowerGrid simulation implements a simplified electrical network solver tracking feeder currents and bus voltages. Frequency is modeled as a single scalar variable adjusted by generation-load imbalance; load‑shedding executes via a priority queue when frequency drops below threshold. Islanding and re‑synchronization are treated as discrete events.

---

## WATER PEA (TO_IMPLEMENT)

**Status**: Critical infrastructure. Specification phase. Ice extraction, water treatment, and distribution for crew hydration, thermal management, agricultural irrigation, and industrial processes.

**Operational Context** (Mars water resources):
- **Purpose**: extract potable water from subsurface ice deposits; treat for trace contaminants (perchlorates, minerals); distribute to crew, ECLSS, Farm, Sabatier feed, and thermal loops
- **Resource**: polar region ice stakes (Underhill base near Dao Vallis, high ice abundance); target extraction rate 50–200 L/sol (5–20 kg/hour continuous)
- **Challenge**: subsurface ice mining in −50°C environment; preventing regolith contamination (perchlorates); maintaining water-loop integrity over 18–24 months; emergency rationing mode if extraction equipment fails
- **Loop closure**: grey water from agriculture/crew recycled through treatment train; black water (sanitation) processed for soil amendment; potable output target 95%+ purity

### Service Set
1. `IceExtraction` — borehole drilling, ice recovery, transport to processing
2. `WaterTreatment` — filtration, contaminant removal (perchlorate, salts), polishing, quality assurance
3. `PotableWaterDistribution` — storage, pressure regulation, emergency ration mode
4. `WastewaterRecycling` — grey/black water treatment, nutrient recovery, discharge sequencing

### Procedure Examples
#### `IceExtraction`
- **`StartingBoreholeHeating`** (BinDrv: `BoreholeMeltHeater`) — melt subsurface ice in situ for extraction
- **`PumpingMeltedWaterToSurface`** (AnaDrv: `WaterInjectionPump`) — pressurized extraction, flow-rate control
- **`SettingExtractedWaterStorageLevel`** (AnaDrv: `StorageTankLevel`) — raw-water staging before treatment

#### `WaterTreatment`
- **`StartingMultistageFilterTrain`** (BinDrv: `FilterCartridgeDrive`) — sediment removal, charcoal polish, ion exchange
- **`SettingTreatmentFlowRate`** (AnaDrv: `TreatmentPumpDrive`) — residence time in beds, purity target
- **`RegeneratingIonExchangeResin`** (BinDrv: `ResineRegenerationHeater`) — periodic caustic/acid cycling for capacity restoration

#### `PotableWaterDistribution`
- **`OpeningDrinkingWaterSupplyLine`** (BinVlv: `PotableSupplyValve`) — crew access to potable reservoir
- **`SettingSupplyPressure`** (AnaDrv: `PressureRegulatorDrive`) — maintain 3–4 bar to all consumers
- **`ActivatingEmergencyWaterRation`** (BinDrv: `RationingValveSolenoid`) — reduce per-capita allocation on shortage

#### `WastewaterRecycling`
- **`CollectingGreyWaterFromAgriculture`** (AnaVlv: `GreyWaterCollectionValve`) — crop runoff, condensate capture
- **`ProcessingBlackWaterForNutrientRecovery`** (BinDrv: `SanitationTreatmentDrive`) — solids separation, biosolids stabilization
- **`ReturningTreatedWaterToECLSSLoop`** (AnaVlv: `RecycleReturnValve`) — close the water cycle

### Water Interlocks (To Be Enforced)
- No drinking-water dispensing if treated-water purity <95% or perchlorate >4 ppb
- Automatic borehole extraction halt if subsurface temperature drops below −20°C (refreezing risk)
- Automatic grey-water routing to agricultural use only; black-water only after full treatment
- No new agricultural irrigation initiations if potable reserve <48 hours supply
- Emergency ration mode activated automatically if extraction rate drops >50% off nominal

### Simulation Model
Water simulation uses mass‑balance equations for storage tanks and extraction rates. Treatment train efficiency and filter saturation are simulated with linear decay curves. Water quality flags and emergency ration transitions are evaluated each tick; recycled flows feed back into ECLSS and Farm simulations.

---

## HABITATSTRUCTURE PEA (TO_IMPLEMENT)

**Status**: Specification phase. Pressure envelope, structural integrity, segmentation, and leak containment for Underhill dome and connected modules.

**Operational Context** (Mars pressure and structural environment):
- **Purpose**: maintain 80 kPa internal cabin pressure against thin Martian atmosphere (~600 Pa); contain or isolate pressure breaches (micrometeorite, material fatigue, dust abrasion); enable evolutionary growth (modular expansion)
- **Architecture**: primary dome (glass/composite layered with regolith berm), secondary airlock airspace, isolated module segments with bulkhead hatches
- **Challenge**: structural loads (14x Earth gravity equivalent pressure differential); thermal cycling (−130°C → −20°C swings); impacts from dust storms; seismic activity (low but non-zero)
- **Safety strategy**: continuous integrity monitoring (acoustic emission, strain gauge networks); automated leak localization; segment-by-segment isolation capability; progressive pressurization to detect small breaches early

### Service Set
1. `PressureEnvelopeMaintenance` — structural monitoring, pressure regulation, leak detection/localization
2. `SegmentIsolation` — bulkhead coordination, pressure differential management, controlled depressurization
3. `ShieldingManagement` — regolith/water shielding maintenance, radiation protection cycling
4. `StructuralHealthMonitoring` — fatigue analysis, repair scheduling, impact event triage

### Procedure Examples
#### `PressureEnvelopeMaintenance`
- **`MonitoringCabinPressure`** (AnaDrv: `PressureSensorArray`) — continuous pressure feedback, leak-rate inference
- **`SettingCabinTargetPressure`** (AnaDrv: `PressureRegulatorDrive`) — 80 kPa nominal, with gradual ramps for crew acclimation
- **`LocatingLeakSourceUsingAcousticEmission`** (AnaDrv: `LeakDetectionArray`) — pinpoint breach location via sound triangulation

#### `SegmentIsolation`
- **`IsOlatingSuspectedLeakSegment`** (BinVlv: `SegmentBulkheadSolenoid`) — rapid isolation on breach detection
- **`DepressurizingSegmentForRepair`** (AnaDrv: `SegmentVentDrive`) — controlled slow depressurization with crew notification
- **`RepressurizingSegmentAfterRepair`** (AnaDrv: `SegmentRepressValve`) — gradual pressurization with leak-rate confirmation

#### `ShieldingManagement`
- **`CyclingWaterShieldingPump`** (AnaDrv: `ShieldWaterCirculationDrive`) — radiation/thermal protection via external water circulation
- **`SettingRegolithBermWaterLevel`** (AnaDrv: `ShieldWaterLevelDrive`) — maintain optimal depth for protection vs. structural load balance

#### `StructuralHealthMonitoring`
- **`RunningFatigueAnalysisOnStressConcentrations`** (BinDrv: `AnalysisSoftwareDrive`) — weekly structural FEA updates
- **`IssuingRepairOrderForHighStrainZones`** (BinDrv: `MaintenanceScheduleDrive`) — proactive reinforcement

### HabitatStructure Interlocks (To Be Enforced)
- No voluntary depressurization if exterior pressure <500 Pa (storm incoming)
- Automatic segment isolation on pressure drop >2 kPa/minute (acute breach)
- No EVA if any segment in leak-suspected state
- Mandatory pressure-cycling test every 30 sols to detect slow leaks (<0.1 kPa/hour)
- Structural integrity check mandatory every 90 sols; no new module connections if standing defects exist

### Simulation Model
HabitatStructure simulation implements a pressure‐leak model using differential equations to update internal pressure based on leak area and volume. A leak localization algorithm uses synthetic acoustic emission timing. Segment isolation is modeled as a binary state machine, and structural health uses a random‑walk fatigue index updated periodically.

---

## FARM PEA (TO_IMPLEMENT)

**Status**: Specification phase. Agricultural production (greenhouse crops, aquaculture, algae bioreactors) for food security and ECLSS atmosphere/water coupling during 18–24 month campaign.

**Operational Context** (Mars agriculture):
- **Purpose**: produce 25–50% of crew caloric needs on-site (backup resupply); harvest water from microalgae and aquaculture condensation; reclaim CO2 from ECLSS exhaust for plant growth; maintain crop diversity for nutritional completeness
- **Challenge**: extreme low light (Mars daylight ~43% Earth); cold nights requiring thermal energy investment; regolith dust contamination; water recycling essential; crop disease/pest risk in closed environment
- **Integration**: tightly coupled to ThermalControl (heating), ECLSS (CO2 source, humidity sink), Water (irrigation, condensate recovery), PowerGen (lighting load ~500 kW peak)
- **Scalability**: modular greenhouse units; start with high-calorie staples (potatoes, soybeans), expand to fruits/vegetables for crew comfort

### Service Set
1. `GreenhouseProduction` — light, humidity, temperature control; crop cycle management
2. `AquacultureProduction` — fish tank management, nutrient cycling, algae supplementation
3. `BioreactorProduction` — algae cultivation for O2, protein, medication precursors
4. `AgriculturalContingency` — emergency crop preservation, pest/disease response, water rationing mode

### Procedure Examples
#### `GreenhouseProduction`
- **`StartingGrowthLightingZone`** (AnaDrv: `LEDArrayDrive`) — activate full-spectrum grow lights, ramp intensity over full-dark period
- **`SettingGreenhouseCO2Enrichment`** (AnaDrv: `CO2InjectionDrive`) — tap ECLSS exhaust, target 800–1200 ppm for optimized photosynthesis
- **`SettingHumiditySetpoint`** (AnaDrv: `HumidificationDrive`) — extract crop transpiration, return to Water loop
- **`ManagingCropCycleTransition`** (BinDrv: `HarvestCoordinationDrive`) — sequence plantings, stagger yields

#### `AquacultureProduction`
- **`StartingRecirculationTank`** (BinDrv: `TankCirculationDrive`) — fish tank aerobiological balance maintenance
- **`SettingNutrientDeliveryRate`** (AnaDrv: `NutrientPumpDrive`) — fish waste → plant fertilizer coupling

#### `BioreactorProduction`
- **`StartingAlgaeCultureBatch`** (BinDrv: `BioreactorAerationDrive`) — bubble aeration, light exposure timing
- **`SettingAlgaeCultivationTemperature`** (AnaDrv: `BioreactorHeatingDrive`) — optimal growth temp ~20–25°C

#### `AgriculturalContingency`
- **`ActivatingEmergencyCropPreservation`** (BinDrv: `DelayedHarvestMode`) — if crew food supply threatened, fast-cycle maturation
- **`InitiatingResponseToPlantDisease`** (BinDrv: `IsolateSuspectedCropDrive`) — containment, emergency replanting signal

### Farm Interlocks (To Be Enforced)
- No CO2 injection into greenhouse if ECLSS ppCO2 drops below 0.2 kPa (life support priority over crop optimization)
- Automatic cooling activation if greenhouse temperature exceeds 35°C (heat stress threshold)
- No new crop plantings if potable water reserves <14 days supply
- Mandatory crop disease monitoring every 7 sols; automatic quarantine on pathogen detection

### Simulation Model
Farm simulation couples a logistic crop‑growth model with environmental inputs (light, CO2, temperature) from ThermalControl and ECLSS. Aquaculture biomass and nutrient cycles are tracked; disease events are probabilistic and trigger containment state machines. Water usage and CO2 uptake feed back into Water and ECLSS loops.

---

## SAFETY PEA (TO_IMPLEMENT, CROSS-CUTTING)

**Status**: Specification phase. Cross-cutting emergency coordination, hazard monitoring, and incident response orchestration affecting all other PEAs.

**Operational Context** (Mars emergency operations):
- **Purpose**: coordinate emergency responses (depress, fire, decompression, dust storm, power loss); manage evacuation if required; orchestrate multi-PEA failover on critical subsystem malfunction; record incident timeline for post-event analysis
- **Integration**: every other PEA exposes mandatory safety interlocks (e.g., "automatically trigger evacuation alarm if Airlock loses pressure control"); SAFETY PEA enforces global precedence
- **Crew criticality**: distributed per-segment emergency kits, tele-triage to medical PEA, safe-haven profiles (minimal O2/CO2/thermal load)
- **Drill cadence**: monthly emergency drills covering depress, fire, EVA-suit-malfunction, and power-loss scenarios

### Simulation Model
Safety PEA simulation is event-driven: hazard variables from all PEAs are sampled each tick and evaluated by a rule engine. Emergency incidents are queued and processed by a state machine that issues override commands and manages evacuation sequences. DrillService uses a scheduler to inject training scenarios, and IncidentRecovery monitors subsystem states to coordinate safe-haven transitions.

### Service Set
1. `EmergencyCoordinationService` — incident declaration, multi-PEA command override, evacuation orchestration
2. `HazardMonitoringService` — real-time hazard detection (flame, toxic gas, pressure loss rate), alarm escalation
3. `DrillService` — scheduled emergency training, competency tracking, post-drill debrief logging
4. `IncidentRecoveryService` — safe-haven mode enforcement, power/thermal minimization, triage support

### Procedure Examples
#### `EmergencyCoordinationService`
- **`DeclaringEmergencyIncident`** (BinDrv: `AlertBroadcastRelay`) — siren activation, crew notification, alert routing to all PEAs
- **`InitiatingEvacuationSequence`** (BinDrv: `EvacuationCommandDrive`) — lock down unsafe segments, guide crew to assembly points
- **`OverridingNormalOperationModeToEnforceSafeHaven`** (BinDrv: `SafeHavenActivationRelay`) — all PEAs switch to minimal-survival posture

#### `HazardMonitoringService`
- **`MonitoringForFlameDetection`** (AnaDrv: `FlameDetectorArray`) — UV/IR sensors in all occupied segments, latency <1 sec
- **`DetectingToxicGasRisks`** (AnaDrv: `GasCompositionMonitor`) — CO, NH3, H2S, CO2 trigger levels defined per contaminant
- **`CalculatingPressureLossRate`** (AnaDrv: `PressureTrendAnalysis`) — infer leak size, forecast time-to-critical

#### `DrillService`
- **`ConductingDepressurization Drill`** (BinDrv: `DrillCoordinationDrive`) — simulated loss of pressurization, crew response timing
- **`ConductingFireResponseDrill`** (BinDrv: `FireResponseScenarioDrive`) — flame detection, evacuation, suppression if applicable
- **`LoggingDrillPerformanceForCrewCompe tencyTracking`** (BinDrv: `DrillRecordingDrive`) — competency matrix updates

#### `IncidentRecoveryService`
- **`ActivatingSafeHavenProfile`** (BinDrv: `SafeHavenModeDrive`) — ECLSS to minimum O2 demand, ThermalControl to baseline, all non-essential loads shed
- **`RoutingTeleTriageRequestToMedicalPEA`** (BinDrv: `MedicalInjury AlertRelay`) — automated injury reporting, remote assessment

### Safety Interlocks (Cross-PEA Integration Required)
- **From Airlock**: Loss of airlock pressure control → automatic Safety alarm + evacuation signal
- **From ECLSS**: CO2 >0.8 kPa OR O2 <16 kPa → automatic alarm, ECLSS override to maximum CO2 removal rate
- **From PowerGen/PowerGrid**: Loss of >75% power → immediate safe-haven mode, Tier 4 loads shed, UPS to Tier 1
- **From HabitatStructure**: Pressure drop >2 kPa/min OR segment leak detected → automatic segment isolation + Safety notification
- **From Farm**: Crop disease detected → quarantine segment, crew healthcare alert
- **Global**: Any PEA in CRITICAL state for >60 seconds → automatic evacuation siren and safe-haven mode unless manually overridden by Incident Commander

---

## Integration & Tier Dependencies

**Tier 1 (Survival)**: Airlock, ECLSS core, HabitatStructure, PowerGen, PowerGrid → must never fail simultaneously
**Tier 2 (Mission)**: Water, ThermalControl, Sabatier, Farm supplemental → enable extended stay and ISRU
**Tier 3 (Optimization)**: Advanced atmosphere, isotope, speciality gas recovery → enable ultra-long missions and science

**Critical Cross-PEA Dependencies**:
- PowerGen/PowerGrid power ALL other systems; loss → controlled shutdown sequence into safe-haven mode
- HabitatStructure integrity required for safe-haven activation (sealed segments to ride out emergency)
- Water + Farm + ECLSS form the life-cycle loop (food, oxygen, water closure)
- Safety enforces global priority and incident precedence across all PEAs

**Status**: Specification phase. Advanced thermal management for Underhill dome and life-support loops to handle Mars cold extremes and maintain crew comfort during 18–24 month campaigns.

**Operational Context** (Mars thermal environment):
- **Purpose**: maintain habitat dome interior at 18.3–26.7°C despite Mars surface extremes (polar region: −130°C night, −20°C peak day); regulate thermal loops for ECLSS, Sabatier reactor, and electronics
- **Challenge**: high diurnal temperature swing (150°C+), minimal atmospheric insulation (600 Pa), solar heating variability, radiator heat rejection to extreme-cold environment
- **Strategy**: dual-path thermal system — active heating via resistive elements during night; radiative cooling + cryogenic heat rejection during day; thermal mass buffering
- **Integration**: tightly coupled to Underhill power subsystem (heating load ~5–15 kW during polar night)

### Service Set
1. `WarmingDome` — active heating to maintain comfortable crew temperature
2. `RegulatingThermalLoops` — thermal management for ECLSS/Sabatier integration
3. `CryogenicCooling` — advanced heat rejection via cryogenic radiators
4. `ThermalContingency` — degraded-mode heating/cooling for power-loss scenarios

### Procedure Model By Service

#### `WarmingDome`
- **`TurningOnHeatingElement`**
  - ControlModule: `PrimaryHeatingElementDrive` (BinDrv)
  - BinaryParameters: `EnableCommand`, `DisableCommand`
  - Reports: `IsHeating`, `ElementTemperatureC`

- **`SettingDomeTargetTemperature`**
  - ControlModule: `DomeThermalDrive` (AnaDrv)
  - AnalogParameters: `TargetTemperatureC`, `MaxHeatingRateKPerMin`
  - Reports: `ActualDomeTemperatureC`, `ProgressToTargetPercentage`, `HeatingPowerKw`

#### `RegulatingThermalLoops`
- **`StartingEclssCirculationPump`**
  - ControlModule: `EclssCirculationDrive` (BinDrv)
  - BinaryParameters: `StartCommand`, `StopCommand`
  - Reports: `IsPumpRunning`, `LoopFlowRateLpm`

- **`SettingLoopTemperatureSetpoint`**
  - ControlModule: `EclssLoopThermalDrive` (AnaDrv)
  - AnalogParameters: `TargetLoopTemperatureC`, `MaxRampRateKPerMin`
  - Reports: `ActualLoopTemperatureC`, `LoopFlowRateLpm`, `ThermalPowerKw`

#### `CryogenicCooling`
- **`IsolatingSuperfluidReservoir`**
  - ControlModule: `CryogenicReservoirValve` (BinVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - Reports: `IsClosed`, `ReservoirPressureBar`

- **`SettingRadiatorCoolantFlowRate`**
  - ControlModule: `RadiatorCoolantDrive` (AnaDrv)
  - AnalogParameters: `FlowRatePercentage`, `MaxCoolingKw`
  - Reports: `ActualFlowLpm`, `RadiatorOutletTemperatureC`, `CoolingEfficiencyPercentage`

#### `ThermalContingency`
- **`ActivatingBackupResistiveHeating`**
  - ControlModule: `BackupHeatingElementDrive` (BinDrv)
  - BinaryParameters: `EnableCommand`
  - Reports: `IsActive`, `OutputKw`

- **`InitiatingReducedPowerThermalMode`**
  - ControlModule: `ThermalModeSwitchDrive` (BinDrv)
  - BinaryParameters: `ActivateCommand`
  - Reports: `DomeSafetyEnvelopeC`, `ThermalModeActive`

### Thermal Control Interlocks (To Be Enforced)
- No continuous heating >50% duty cycle without radiator coolant pressure >2.0 bar
- Auto-activate backup heating if dome temperature drops below 15°C
- Auto-activate cryogenic cooling if radiator exceeds 40°C
- No heating element power increase if sheath temperature >200°C
- Automatic degraded-mode activation on loss of primary circulation pump

### Simulation Model
ThermalControl uses a dual‑mode RC thermal network solver. Dome and loop temperatures are computed each tick with heating elements and cryogenic cooling represented as controllable heat sources/sinks. The model draws power from PowerGen and provides temperature feedback to other PEA simulations. Emergency contingencies transition to backup heating automatically.

---

## NOBLE GAS RECOVERY PEA (TO_IMPLEMENT)

### Simulation Model
NobleGasRecovery simulation maintains a mass balance on intake flow and separation efficiency. Cryogenic cycle state machine alternates between cold and regeneration modes, with product accumulation computed each tick. Membrane selection analog control affects yield and purity.


**Status**: Specification phase. Argon/neon/krypton extraction and purification for breathing-mix adjustment, pressure compensation, and future ISRU applications.

**Operational Context** (Mars atmosphere ISRU):
- **Purpose**: extract trace noble gases from thin Martian atmosphere (96% CO2, ~2.7% N2, ~1.6% Ar, ~0.13% O2, <0.1% Ne/Kr); purify to support crew breathing comfort and ISRU chemistry
- **Mission application**: breathing-mix tuning (add Ar to dilute O2 for comfort during high-pressure cycles); oxygen-equivalent compensation for suit pressurization
- **Scale**: low-volume production (~100–500 mL/sol at Mars sample rate), high storage density
- **Integration**: intake from Mars external atmosphere; output to breathing-gas storage or Sabatier feed

### Service Set
1. `GasIntake` — atmospheric sampling and intake management
2. `NobleGasExtraction` — cryogenic or membrane-based separation
3. `PurificationAndStorage` — product polishing and high-pressure storage

### Procedure Model By Service

#### `GasIntake`
- **`OpeningMarsAtmosphereIntakeValve`**
  - ControlModule: `AtmosphereIntakeValve` (AnaVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - AnalogParameters: `OpenPercentage`
  - Reports: `ActualOpenPercentage`, `IntakePressurePa`, `IntakeFlowRateCfh`

- **`SettingIntakePressureSetpoint`**
  - ControlModule: `IntakePressureDrive` (AnaDrv)
  - AnalogParameters: `TargetPressureBar`, `MaxFlowRateCfh`
  - Reports: `ActualPressureBar`, `ActualFlowCfh`, `DownstreamCondensationRiskPercentage`

#### `NobleGasExtraction`
- **`StartingCryogenicSeparationUnit`**
  - ControlModule: `CryoSeparationDrive` (BinDrv)
  - BinaryParameters: `StartCommand`, `StopCommand`
  - AnalogParameters: `CooldownTargetK`
  - Reports: `IsOperational`, `ChamberTemperatureK`, `ArgonRecoveryPercentage`

- **`SettingNobleGasSelectionFilter`**
  - ControlModule: `SelectiveMembraneValve` (AnaVlv)
  - AnalogParameters: `MembraneSelectivityPercentage`, `TargetGasCompositionCo2Ar`
  - Reports: `ActualSelectivityPercentage`, `ProductPurityPercentage`, `RejectionFlowRateCfh`

#### `PurificationAndStorage`
- **`ActivatingProductPolishingBed`**
  - ControlModule: `PolishingBedDrive` (BinDrv)
  - BinaryParameters: `ActivateCommand`, `DeactivateCommand`
  - Reports: `IsPurifying`, `RemovalEfficiencyPercentage`, `BedSaturationPercentage`

- **`CompressingNobleGasToStorage`**
  - ControlModule: `StorageCompressionDrive` (AnaDrv)
  - BinaryParameters: `StartCommand`, `StopCommand`
  - AnalogParameters: `TargetStoragePressureBar`, `MaxCompressionRateCfh`
  - Reports: `StoragePressureBar`, `CompressedProductMass_g`, `CompressionEfficiencyPercentage`

### Noble Gas Recovery Interlocks (To Be Enforced)
- No external intake if cold-trap temperature >−50°C (condensation risk)
- Purging system before gas-selection membrane activation
- No product compression if purity <98% noble gas
- Auto-shutdown and purge on pressure-relief activation

---

## ISOTOPE FRACTIONATION PEA (TO_IMPLEMENT)

### Simulation Model
IsotopeFractionation simulation treats enrichment as a batch process with discrete concentration updates per cycle. A state machine governs start/pause/regenerate actions; purity calculations use simple ratio equations. Fault injection includes purge failures and power interruptions.


**Status**: Specification phase. Oxygen-18 and deuterium fractionation for future Mars science objectives and advanced ISRU chemistries.

**Operational Context** (Mars science & ISRU evolution):
- **Purpose**: separate stable isotopes (O-18/O-16, D/H) from habitat atmosphere for scientific analysis or specialized industrial chemistry
- **Application path**: Phase 2+ capability for extended missions requiring isotopic tracers or isotope-dependent reaction kinetics (e.g., advanced fuel synthesis, biomedical research)
- **Separation method**: centrifugal or laser-assisted isotope enrichment (LAIE) with staged recycling
- **Output scale**: gram-scale quantities per sol for science or dedicated ISRU streams
- **Integration**: non-critical to core life support; graceful isolation on malfunction

### Service Set
1. `IsotopeEnrichment` — separation and concentration
2. `ProductRecycling` — recirculation of partially-enriched streams to boost concentration
3. `IsotopeContingency` — safe shutdown and cleanout

### Procedure Model By Service

#### `IsotopeEnrichment`
- **`StartingCentrifugalEnrichmentUnit`**
  - ControlModule: `CentrifugeDrive` (AnaDrv)
  - BinaryParameters: `StartCommand`, `StopCommand`
  - AnalogParameters: `SpinRateRpm`, `EnrichmentTemperatureC`
  - Reports: `IsOperational`, `ActualRpm`, `EnrichmentFactorOxygen18`, `ProductFlowCfh`

- **`SettingIsotopeTargetConcentration`**
  - ControlModule: `EnrichmentControlDrive` (AnaDrv)
  - AnalogParameters: `TargetO18PercentCo2Depletion`, `TargetDeuteriumPercentage`
  - Reports: `ActualO18PercentEnrichment`, `ActualDeuteriumPercentEnrichment`, `RecycleRatePercentage`

#### `ProductRecycling`
- **`OpeningRecycleLoopValve`**
  - ControlModule: `RecycleValve` (AnaVlv)
  - AnalogParameters: `OpenPercentage`, `TargetRecycleRatioPercentage`
  - Reports: `ActualOpenPercentage`, `RecycleFlowRateCfh`, `FractionationEfficiencyPercentage`

#### `IsotopeContingency`
- **`InitiatingSafeShutdownAndPurge`**
  - ControlModule: `ShutdownPurgeDrive` (BinDrv)
  - BinaryParameters: `ShutdownCommand`, `PurgeCommand`
  - Reports: `ShutdownInitiated`, `SystemClean`, `ResidualsRecovered_g`

### Isotope Fractionation Interlocks (To Be Enforced)
- No enrichment operation if main ECLSS branch isolation valve not confirmed open/safe
- Auto-isolation if product pressure exceeds safe limits
- Mandatory system purge before extended shutdown to prevent isotope stratification
- Block new enrichment cycle if residual enrichment >5% to prevent compound enrichment errors

---

## ADVANCED ATMOSPHERE POLISHING PEA (TO_IMPLEMENT)

### Simulation Model
AdvancedAtmospherePolishing simulation tracks media saturation linearly with processed volume; catalytic efficiency decays with temperature and time. Regeneration cycles are implemented as timed state machines. Bypass logic is triggered when differential pressure exceeds threshold.


**Status**: Specification phase. Catalytic and multi-stage contaminant removal beyond ECLSS baseline; extends atmosphere loop life and crew health during ultra-long missions.

**Operational Context** (Extended Mars residence):
- **Purpose**: remove trace organics, sulfur compounds, nitrogen oxides, and other contaminants beyond ECLSS scrubber capability; maintain air quality for 18–24 month crew exposure
- **Challenge**: Mars dust ingestion (perchlorate salts, iron oxide), metabolic trace compounds, equipment off-gassing, microbial volatiles; ECLSS alone may not achieve cleanup at mission timescales
- **Technology**: catalytic oxidation (heated Pt/Pd beds), granular activated carbon, ionic exchangers, high-capacity particulate pre-filters
- **Integration**: branch off ECLSS outlet, pass through advanced beds, return to habitat cabin intake

### Service Set
1. `CatalyticOxidation` — thermal catalytic removal of volatile organics
2. `AbsorbentPolishing` — granular carbon and ionic removal of residuals
3. `ParticleManagement` — pre-filtering and nano-particulate capture

### Procedure Model By Service

#### `CatalyticOxidation`
- **`ActivatingCatalyticBed`**
  - ControlModule: `CatalyticHeaterDrive` (BinDrv)
  - BinaryParameters: `ActivateCommand`, `DeactivateCommand`
  - AnalogParameters: `SetpointTemperatureC`
  - Reports: `IsActive`, `BedTemperatureC`, `OxidationEfficiencyPercentage`

- **`SettingOxidationAirFlow`**
  - ControlModule: `CatalyticFlowDrive` (AnaDrv)
  - AnalogParameters: `FlowRateCfh`, `ResidenceTimeMs`
  - Reports: `ActualFlowCfh`, `BypassFlowCfh`, `ContaminantRemovalRatePercentage`

#### `AbsorbentPolishing`
- **`EngagingActivatedCarbonStage`**
  - ControlModule: `CarbonBedDrive` (BinDrv)
  - BinaryParameters: `EngageCommand`, `DisengageCommand`
  - Reports: `IsEngaged`, `BedSaturationPercentage`, `BreakthroughRiskPercentage`

- **`RegeneratingAbsorbentMedia`**
  - ControlModule: `MediaRegenerationDrive` (AnaDrv)
  - BinaryParameters: `StartRegenerationCommand`
  - AnalogParameters: `RegenerationTemperatureC`, `RegenerationDurationMinutes`
  - Reports: `RegenerationInProgress`, `DesorbedContaminantMass_mg`, `MediaCapacityRestored`

#### `ParticleManagement`
- **`InstallingParticleDuct`**
  - ControlModule: `DuctConfigurationDrive` (BinDrv)
  - BinaryParameters: `InstallCommand`, `RemoveCommand`
  - Reports: `DuctInstalled`, `ParticleLoadMilygrams`, `BypassDifferentialPa`

- **`SettingNanoParticleCaptureFinenesss`**
  - ControlModule: `FilterFinenessDrive` (AnaDrv)
  - AnalogParameters: `FilterMicronRating`, `MaxAirFlowWithoutBypass`
  - Reports: `ActualCaptureMicronRating`, `ParticleCaptureEfficiencyPercentage`, `FilterDeltaPa`

### Advanced Polishing Interlocks (To Be Enforced)
- No catalytic operation if bed temperature <100°C (cold-start safety)
- Automatic bypass if polishing stage ΔP exceeds 10 cmH2O (media saturation)
- Mandatory media regeneration cycle every 30 days or 72-hour operation, whichever first
- Auto-isolation of polishing branch if bypass flow >20% nominal

---

## GAS STORAGE & DISTRIBUTION PEA (TO_IMPLEMENT)

### Simulation Model
GasStorageAndDistribution simulation updates tank pressures using the ideal gas law based on inflow/outflow commands. Flow distribution uses a priority metering algorithm; emergency vent depressurization occurs at a fixed rate when commanded. Inventory tracking and leak detection run each tick.

**Status**: Specification phase. High-pressure storage and precision distribution of specialty gases (noble gases, emergency oxygen, isotope-enriched streams) supporting Underhill operations.

**Operational Context** (Mars ISRU supply chain):
- **Purpose**: serve as the central collection, storage, and distribution hub for all specialty-gas streams (ThermalControl cryogenic buffer, NobleGasRecovery product, IsotopeFractionation output); provide emergency oxygen backup
- **Design**: modular high-pressure vessels, solenoid isolation, automated pressure regulation, supply rerouting for contingencies
- **Capacity**: sized for 60-day nominal operation plus 30-day emergency contingency without production
- **Safety**: triple-redundant isolation, rupture-disk overpressure protection, quarterly inspection certification

### Service Set
1. `GasReception` — intake from production subsystems and pressure regulation
2. `StorageManagement` — inventory tracking, pressure cycling, material compatibility
3. `PrecisionDistribution` — metered gas supply to habitat loads and contingency consumers
4. `StorageContingency` — emergency isolation and pressure relief

### Procedure Model By Service

#### `GasReception`
- **`IsolatingSectionInlet`**
  - ControlModule: `InletIsolationSolenoid` (BinVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - Reports: `IsOpen`, `UpstreamPressureBar`

- **`SettingReceptionPressureRegulator`**
  - ControlModule: `ReceptionRegulatorDrive` (AnaDrv)
  - AnalogParameters: `RegulatedDownstreamPressureBar`, `MaxFlowRateCfh`
  - Reports: `ActualDownstreamPressureBar`, `ActualFlowCfh`, `RegulatorsHealthy`

#### `StorageManagement`
- **`SettingStorageVesselPressure`**
  - ControlModule: `VesselPressureMonitorDrive` (AnaDrv)
  - AnalogParameters: `TargetPressureBar`, `MaxPressureBar`
  - Reports: `VesselPressureBar`, `VesselTemperatureC`, `StoredMassEstimate_kg`, `RuptureDiskIntegrityOk`

- **`StartingPressureCyclingForMaterialHealth`**
  - ControlModule: `PressureCycleDrive` (BinDrv)
  - BinaryParameters: `StartCycleCommand`, `StopCycleCommand`
  - AnalogParameters: `CycleBandBar`
  - Reports: `CycleInProgress`, `CyclesCompleted`, `VesselFatigueIndex`

#### `PrecisionDistribution`
- **`OpeningDistributionValve`**
  - ControlModule: `DistributionSolenoid` (BinVlv)
  - BinaryParameters: `OpenCommand`, `CloseCommand`
  - Reports: `IsOpen`, `DownstreamDemandBar`

- **`SettingPrecisionMeteringFlow`**
  - ControlModule: `MeteringRegulatorDrive` (AnaDrv)
  - AnalogParameters: `TargetFlowCfh`, `MaxPressureDrop_cmH2O`
  - Reports: `ActualFlowCfh`, `FlowAccuracyPercentage`, `DownstreamConsumerPressureBar`

#### `StorageContingency`
- **`ActivatingEmergencyOxygenSupply`**
  - ControlModule: `EmergencyO2SolenoidDrive` (BinDrv)
  - BinaryParameters: `EnableCommand`, `DisableCommand`
  - Reports: `IsSupplying`, `EmergencyO2PressureBar`, `TimeToExhaustionHours`

- **`InitiatingReleaseVentToPurgeSystem`**
  - ControlModule: `ReleaseVentDrive` (BinDrv)
  - BinaryParameters: `OpenVentCommand`
  - Reports: `VentInitiated`, `DepressurizationRateBar_Per_Min`, `SystemSafeForMaintenance`

### Gas Storage & Distribution Interlocks (To Be Enforced)
- No reception flow if vessel pressure >95% of rupture-disk setpoint
- Automatic emergency O2 activation if habitat ppO2 drops below 16 kPa for >30 seconds
- No precision metering to non-essential loads if vessel pressure <50% of rated
- Mandatory quarterly rupture-disk and isolation-valve certification
- Force contingency vent on any subsystem malfunction preventing normal outlet shutdown

---

# Underhill Base Components & Architecture

## Physical Layout & Systems Integration

**Underhill** is a Mars surface habitat base designed for sustained crew presence and in-situ resource utilization (ISRU). The base integrates four primary subsystem packages, each with multi-service operational architecture.

### Airlock Module (Operational)
The primary crew ingress/egress pressure boundary. Interfaces with habitat cabin via inner hatch (to pressurized crew areas) and external Mars surface via outer hatch (to ~600 Pa Martian atmosphere).
- **Function**: enables crew EVA at variable cabin pressures, reclaims atmosphere during repressurization, supports dual-phase dust mitigation (inner dust/decontamination, outer abrasive-particle management)
- **Control systems**: six service classes (Depressurizing, Pressurizing, HatchTransfer, AtmosphereReclaim, EmergencyRepressurizing, Isolation) managing pressure cycles, hatch locking, emergency repress, and atmosphere recovery
- **Hardware**: vacuum pump, pressure relief and supply valves, motorized inner/outer hatches with independent locks, reclaim/isolation solenoid plumbing, one-way check valves
- **Operational tempo**: supports aggressive EVA schedule (~24 hrs/week crew time during surface campaign phase), multiple sorties per day
- **Safety criticality**: loss of airlock sealing = loss of habitat pressurization; interlocks prevent hatch opening under pressure mismatch

### ECLSS Module (To Implement)
Closed-loop atmosphere and water revitalization. Removes CO2, generates O2 via electrolysis, processes wastewater, manages temperature/humidity.
- **Function**: maintains breathable atmosphere within NASA limits (ppO2 18.0–23.8 kPa, ppCO2 ≤0.53 kPa time-averaged), produces potable water from metabolic and equipment condensate, regulates thermal environment against Mars cold (polar region: −130°C night/−20°C day) and intense solar swings
- **Control systems**: five service classes (Co2Removal, OxygenGeneration, WaterRecovery, AtmosphereConditioning, ContingencyLifeSupport) for CO2 scrubber management, electrolyzer operation, water processing, thermal exchanger control, and safe-haven profiles
- **Hardware**: CO2 scrubber beds (CDRA-equivalent or equivalent), water electrolyzer with hydrogen/oxygen separation, water multifiltration processor, thermal exchangers (radiators + cryogenic heat rejection), circulation pumps and fans, trace-contaminant removal cartridges
- **Integration constraints**: must operate under varying power (solar array output ±60% daily swing); handles high CO2 load post-EVA (~1200 ppm spike from crew exertion); maintains redundant CO2/O2 paths for critical functions; water-loop closure enforced (no consumable water loss to dust/leakage)
- **Crew impact**: ECLSS failure → rapid breach of NASA atmospheric limits → crew incapacitation in minutes; loop-closure efficiency directly correlates with mission duration before resupply

### Sabatier Module (To Implement)
Methanation reactor producing methane fuel for return vehicle, powered by Underhill solar/power subsystem. Hydrogen recovery loop supports ECLSS electrolyzer, closing a critical consumables loop.
- **Function**: converts atmospheric CO2 (captured from Martian air) + recycled H2 (from ECLSS electrolyzer feedback) → CH4 (fuel for Mars Ascent Vehicle) + H2O (return to ECLSS water reclaim); closes hydrogen cycle; enables in-situ propellant production (In-Situ Resource Utilization, propellant ISRU)
- **Control systems**: five service classes (Methanation, FeedConditioning, HydrogenRecovery, CatalystRegeneration, SabatierContingency) managing feedstock stoichiometry, reactor thermodynamics, hydrogen separation, catalyst health cycling, emergency shutdown with venting
- **Hardware**: reactor vessel with Ni-based catalyst bed (~250–450°C operation, ~1–10 bar nominal), feed conditioning/drying train, hydrogen recovery compressor, post-processor for product separation (CH4/H2O/unreacted feedstock), regeneration heating elements
- **Integration constraints**: energy-intensive (large power draw); operates in load-following mode with Underhill power subsystem; tight stoichiometric control (H2:CO2 ratio ~4:1) required for efficiency and catalyst health; catalyst regeneration cycles (thermal/pressure swing) required weekly/monthly depending on throughput; safe-haven depressurization logic on high pressure or impurity threshold; tight coupling to ECLSS oxygen cycle (O2 depletion → H2 undersupply → methane production failure)
- **Mars coupling**: uses Mars CO2 atmosphere (96% CO2 at ~600 Pa); intake from external atmosphere; potential future coupling to ice-mining subsystem for advanced propellant synthesis
- **Criticality**: methane production directly enables crew return vehicle refueling; loss of Sabatier capability → extended stay or mission abort dependency on resupply

### Advanced Subsystem Family (ThermalControl, NobleGasRecovery, IsotopeFractionation, AdvancedAtmospherePolishing, GasStorageAndDistribution) — To Implement, Phases 4–5

**Integrated Advanced Operations** supporting Underhill's evolution from Phase 1 (survival) → Phase 2+ (extended mission sustainability and science).

- **ThermalControl**: Advanced heating/cooling to manage Mars extremes (−130 to −20°C swing polar night) and support all downstream systems; dual-path architecture (active resistive heating + cryogenic radiators)
- **NobleGasRecovery**: Mars atmospheric extraction of Ar/Ne/Kr for breathing-mix tuning and future ISRU; low-volume but high-value specialty gas production
- **IsotopeFractionation**: Stable isotope enrichment (O-18/D) for Mars science (atmospheric composition studies, biomedical isotope tracers) and future advanced chemistry
- **AdvancedAtmospherePolishing**: Catalytic and absorbent cleanup beyond ECLSS baseline; supports ultra-long crew stays (18–24 months) with minimal air-quality drift
- **GasStorageAndDistribution**: Central hub collecting all specialty-gas streams (thermal buffers, noble-gas product, isotope output, emergency O2 reserves) with precision metering to consumers

**Integration Philosophy**: Each subsystem operates **independently** yet **cooperatively**. No single subsystem malfunction cascades to core life support (Airlock/ECLSS/Sabatier). All advanced subsystems feature graceful degradation and automatic isolation on fault detection.

---

# Data Structure Patterns & Implementation Guidance

## Canonical PEA/Service/Procedure Layout

Use this YAML pattern for architecture documentation and code scaffolding:

```yaml
Pea: Airlock
  Service: Depressurizing
    ServiceStateMachine:
      CurrentState: Idle
      ActiveProcedure: OpeningPressureReleaseValve
      TimeInState_sec: 0.0
    Modes:
      OperationMode: Auto
      SourceMode: Operator
      CommandEn: true
      CommandEnReason: ""
    ProcedureSet:
      - ProcedureName: OpeningPressureReleaseValve
        ProcedureId: 1
        ControlModule: PressureReleaseValve
        ControlModuleType: AnaVlv
        BinaryParameters:
          - Name: OpenCommand
            Value: false
          - Name: CloseCommand
            Value: false
        AnalogParameters:
          - Name: OpenPercentage
            Unit: "%"
            Value: 0.0
            Min: 0.0
            Max: 100.0
        Reports:
          - Name: ActualOpenPercentage
            Unit: "%"
            Value: 0.0
          - Name: PressureDropRateKpaPerSec
            Unit: "kPa/s"
            Value: 0.0
```

## OPC UA Browse Path Pattern

All Airlock+ECLSS+Sabatier+NexusAurora nodes follow this hierarchy:

```
Objects/OldTobeyFarms/UnderHillBase/{PeaName}/ServiceSet/{ServiceName}/
  ├─ ServiceInformation/
  ├─ Modes/
  │  ├─ OperationMode
  │  ├─ SourceMode
  │  ├─ CommandEn
  │  └─ CommandEnReason
  ├─ StateMachine/
  │  ├─ CurrentState
  │  ├─ TimeInState_sec
  │  └─ BlockingCondition
  └─ ProcedureSet/
     └─ {ProcedureName}/
        ├─ ControlModule/{ControlModuleName}/
        │  ├─ BinaryParameters/
        │  │  ├─ OpenCommand
        │  │  ├─ CloseCommand
        │  │  └─ ...
        │  ├─ AnalogParameters/
        │  │  ├─ OpenPercentage
        │  │  ├─ TargetPressureKpa
        │  │  └─ ...
        │  └─ Reports/
        │     ├─ ActualOpenPercentage
        │     ├─ PressureDropRateKpaPerSec
        │     └─ ...
        ├─ Parameters/ (procedure-level input)
        ├─ Status/
        │  ├─ State
        │  └─ ProgressPercentage
        └─ ReportValues/ (procedure-level output)
           ├─ LastExecutionResult
           └─ ExecutionTimestamp_ms
```

## REST API Service Definition Pattern

Per `/api/v2/pea/{pea_id}/services/{service_name}`:

```json
{
  "id": 1,
  "name": "Depressurizing",
  "parameters": [
    {
      "name": "TargetPressureKpa",
      "category": "ProcedureParameter",
      "data_type": "xs:double",
      "unit": "kPa",
      "min": 0.0,
      "max": 150.0,
      "default_value": 3.5,
      "description": "Target airlock pressure"
    }
  ],
  "procedures": [
    {
      "id": 1,
      "name": "OpeningPressureReleaseValve",
      "description": "Vent chamber to reduce pressure",
      "input_parameters": ["TargetPressureKpa"],
      "output_parameters": ["FinalPressureKpa"]
    }
  ]
}
```

---

# Transition & Implementation Roadmap

## Phase 1: Airlock (COMPLETE)
✅ Runtime command binding (`START_DEPRESSURIZE_CYCLE` → `Depressurizing` service)
✅ Procedure runtime tracking (state, progress, results)
✅ OPC UA nodes with control-module-aware structure
✅ REST API service definitions

## Phase 2: Utilities Backbone (TO_DO)
- [ ] **PowerGen**: Implement 4 services + 12+ procedures for solar, nuclear, storage, backup power
- [ ] **PowerGrid**: Implement 4 services + 10+ procedures for distribution, load shedding, grid stability
- [ ] **Water**: Implement 4 services + 10+ procedures for ice extraction, treatment, distribution, recycling
- [ ] **ThermalControl**: Implement 4 services + 8+ procedures with dual heating/cooling paths
- [ ] Power supply contracts defined for all downstream PEAs
- [ ] Water loop closure logic for ECLSS/Farm/Sabatier integration
- [ ] Thermal coupling of all heat-generating subsystems (Sabatier, PowerGen, ECLSS)
- [ ] Dashboard and OPC UA endpoints live for utilities oversight

## Phase 3: Habitat Structure & Emergency Systems (TO_DO)
- [ ] **HabitatStructure**: Implement 4 services + 10+ procedures for pressure, isolation, monitoring, shielding
- [ ] **Safety**: Implement 4 services + 12+ procedures for emergency coordination, hazard monitoring, drills, recovery
- [ ] Automated segment isolation and containment logic for breaches
- [ ] Mandatory monthly emergency drill schedule with post-event telemetry capture
- [ ] Integration of safety interlocks from all Tier 1 PEAs (Airlock, ECLSS, Water, PowerGen, PowerGrid, HabitatStructure)
- [ ] Safe-haven mode fully automated across all subsystems

## Phase 4: ECLSS and Propellant Production (TO_DO)
- [ ] **ECLSS**: Implement 5 services + 20+ procedures with power/water dependencies
- [ ] **Sabatier**: Implement 5 services + 11+ procedures with thermal and H2 recovery feedback
- [ ] Closed-loop loop-closure tracking for water and consumables
- [ ] Degraded-mode state machines for continued operation under component failures

## Phase 5: Agricultural & Advanced Operations (TO_DO)
- [ ] **Farm**: Implement 4 services + 10+ procedures with ECLSS/Thermal/Water integration
- [ ] **NobleGasRecovery**: 3 services + 7+ procedures for argon/neon extraction and storage
- [ ] **IsotopeFractionation**: 3 services + 6+ procedures for O-18/deuterium separation
- [ ] **AdvancedAtmospherePolishing**: 3 services + 8+ procedures for catalytic and absorbent cleanup
- [ ] **GasStorageAndDistribution**: 4 services + 10+ procedures for hub operations and contingency metering
- [ ] 30-sol resilience scenario with food/water/medical continuity under fault injection
- [ ] Cross-subsystem isolation logic and fault propagation rules

## Implementation Guidelines
1. **Phased integration order is critical**: Phases 1–3 form the survivability backbone; Phases 4–5 extend mission capability. Do not skip ahead.
2. **Keep existing runtime command IDs** until full MTP refactor; map enum names to PascalCase only.
3. **Build compatibility layer** from old snake_case to new PascalCase names before UI/API changes.
4. **Refactor in order**: service tags → procedure names → parameter names → OPC UA paths → API payloads.
5. **Test each service** with MTP state machine compliance before moving to next subsystem.
6. **Cross-PEA testing**: After each phase, run integrated test suite exercising inter-PEA dependencies (e.g., ECLSS power draw subject to PowerGrid shedding logic).
7. **Reference this document** as the single source of truth for developers; no parallel naming specs.

---

# References & Resources

## MTP Standards
- VDI/VDE/NAMUR 2658-1, 2658-3, 2658-4 (MTP profile & automation services)
- OPC UA (node structure, security policies, procedural calling conventions)

## NASA Standards & Technical Reports
1. NASA-STD-3001/HIDH Appendix E Vol 2 — Crew environment requirements
2. NASA ECLSS reference: https://www.nasa.gov/reference/environmental-control-and-life-support-systems-eclss/
3. NASA SCOR (oxygen recovery beyond Sabatier): https://www.nasa.gov/spacecraft-oxygen-recovery-scor/
4. NASA NTRS Hydrogen Purification (2013): https://ntrs.nasa.gov/archive/nasa/casi.ntrs.nasa.gov/20130011855.pdf
5. NASA NTRS Methane Post-Processor development (2014): https://ntrs.nasa.gov/citations/20140002592
6. NASA NTRS CDRA context & CO2 control (2023): https://ntrs.nasa.gov/citations/20230016796
7. NASA NTRS Airlock Architecture (2022): https://ntrs.nasa.gov/citations/20220016131
8. NASA NTRS Space Station Airlock baseline (1986): https://ntrs.nasa.gov/archive/nasa/casi.ntrs.nasa.gov/19870003734.pdf
9. NASA NTRS EVA Suitport operations (2016): https://ntrs.nasa.gov/api/citations/20160010799/downloads/20160010799.pdf
10. NASA MOXIE mission completion: https://www.nasa.gov/solar-system/nasas-oxygen-generating-experiment-moxie-completes-mars-mission/
11. NASA Mars atmosphere context: https://science.nasa.gov/resource/the-five-most-abundant-gases-in-the-martian-atmosphere

## ISRU & Sabatier References
- LPI-002142 (lunar propellant ISRU architecture, automation, redundancy patterns): https://www.lpi.usra.edu/lpi/contribution_docs/LPI-002142.pdf
- CO2 methanation thermodynamics: https://www.mdpi.com/2504-3900/133/1/1

## Implementation Codebase
- `backend/src/model.rs` — Service/Procedure/ControlModule data structures
- `backend/src/sim.rs` — Simulation engine, state machines, procedure execution
- `backend/src/opcua.rs` — OPC UA server, node hierarchy, snapshot synchronization
- `backend/src/main.rs` — REST API v2 endpoints for service/procedure definitions
- `frontend/app.js` — HMI panel integration, command dispatch, snapshot visualization
