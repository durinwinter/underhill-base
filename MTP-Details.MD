# MTP Details (Part 1 + Part 4 + Underhill Implementation)

## Scope
This document consolidates:
- `MTP_Part-1_Overview_30002_d10_June25.pdf` (concept/model architecture)
- `MTP_Part-4_ Automation_Services_30032_d09_May25 - Copy.pdf` (Automation Services + Process Values)
- Underhill implementation code in `mars-airlock-sim/backend/*`

The goal is a practical data model for services/procedures/object types, then a derived PEA model for the 3 implemented subsystems.

Related remodel using PascalCase and explicit Service/Procedure semantics:
- `MTP-Should-Model-Realigned.MD`

## Part 1 anchors (architecture constraints)

From Part 1:
- MTP integrates **PEAs** into a **POL** via standardized interfaces/model objects.
- MTP is structured as **aspects** and **profiles**.
- Aspect/profile identity format: `ModuleTypePackage:{AspectTitle}.{ProfileTitle}:{Version}`.
- Manifest is the root aspect:
  - IH `ModuleTypePackage` as central entry point.
  - Exactly one IE of SUC `ModuleTypePackage` in that IH.
  - Aspect table of contents via SUC `MTPSet` entries.
  - Each `MTPSet` entry references an aspect IH through IC `AspectSetReference.AspectRef`.
- Cross-aspect linking is done with SUC `LinkedObject` + `RefID` (GUID string):
  - DataAssembly objects are LinkedObjects.
  - `RefID` is the key for joining model objects across aspects.
  - DataAssembly `RefID`s shall be unique in an MTP.

## Part 4 profile map

Automation Services:
- `ModuleTypePackage:ServiceSet.Base:2.0.0`
- `ModuleTypePackage:ServiceSet.ComplexTypes:2.0.0`

Process Values:
- `ModuleTypePackage:ProcessValueSet.Base:2.0.0`
- `ModuleTypePackage:ProcessValueSet.ComplexTypes:2.0.0`

## Part 4 service/procedure control model

### Service mode control keys (Table 2)
Key variables in ServiceMode model:
- Channel selectors: `StateChannel`, `SrcChannel`
- Mode requests: `StateOffAut`, `StateOpAut`, `StateAutAut`, `StateOffOp`, `StateOpOp`, `StateAutOp`
- Active mode flags: `StateOpAct`, `StateAutAct`, `StateOffAct`
- Source requests: `SrcExtAut`, `SrcIntAut`, `SrcIntOp`, `SrcExtOp`
- Active source flags: `SrcIntAct`, `SrcExtAct`

### State machine keys (16 states, Table 3)
State/status integer bit values:
- `Stopped=4`, `Starting=8`, `Idle=16`, `Paused=32`, `Execute=64`, `Stopping=128`, `Aborting=256`, `Aborted=512`, `Holding=1024`, `Held=2048`, `Unholding=4096`, `Pausing=8192`, `Resuming=16384`, `Resetting=32768`, `Completing=65536`, `Completed=131072`

Command/control integer bit values:
- `Reset=2`, `Start=4`, `Stop=8`, `Hold=16`, `Unhold=32`, `Pause=64`, `Resume=128`, `Abort=256`, `Restart=512`, `Complete=1024`

### Procedure model constraints
- Each Service shall have at least one Procedure.
- Service can run only one Procedure at a time.
- `ProcedureID=0` means no Procedure selected.
- Procedure control channels: `ProcedureOp`, `ProcedureInt`, `ProcedureExt`.
- Procedure feedback: `ProcedureCur`, `ProcedureReq`.
- Procedure health includes `CommandInfo` (same encoding family as `CommandEn`).
- Procedure attributes include `IsSelfCompleting`, `ProcedureID`, `Classification`.

## Part 4 object-type catalog (all major object types + key values)

### DataAssembly object types
- `ServiceElement` (SUC): `WQC`
- `ServiceControl` (SUC):
  - control words: `CommandOp`, `CommandInt`, `CommandExt`
  - procedure select: `ProcedureOp`, `ProcedureInt`, `ProcedureExt`
  - feedback: `StateCur`, `CommandEn`, `ProcedureCur`, `ProcedureReq`
  - interaction/position: `PosTextID`, `InteractQuestionID`, `InteractAnswerID`, `InteractAddInfo`
  - service mode/source mode: all `State*`/`Src*` variables
  - value assignment: `ProcParamApplyEn/Ext/Op/Int`, `ConfigParamApplyEn/Ext/Op/Int`
  - report freeze: `ReportValueFreeze`
- `ProcedureHealthView` (SUC): `CommandInfo`
- `ParameterElement` (SUC): `OSLevel`, `ApplyEn`, `ApplyExt`, `ApplyOp`, `ApplyInt`, `Sync`, plus all `State*`/`Src*` mode keys
- `AnaServParam` (SUC): `VExt`, `VOp`, `VInt`, `VReq`, `VOut`, `VFbk`, `VSclMin`, `VSclMax`, `VUnit`, `VMin`, `VMax`
- `DIntServParam` (SUC): same shape as `AnaServParam` but DINT types
- `BinServParam` (SUC): `VExt`, `VOp`, `VInt`, `VReq`, `VOut`, `VFbk`, `VState0`, `VState1`
- `StringServParam` (SUC): `VExt`, `VOp`, `VInt`, `VReq`, `VOut`, `VFbk`
- `MissedValueFlag` (RC): `MissedValue`
- `IndicatorElement` (SUC): `WQC` (+ optional `MissedValueFlag`)
- `StructServParam` (SUC): `VExt`, `VInt`, `VOp`, `VReq`, `VOut`, `VFbk`, `VType`
- `ArrayServParam` (SUC): `IndexExt`, `IndexInt`, `IndexOp`, `IndexMin`, `IndexMax`, `IndexCur`, `VExt`, `VInt`, `VOp`, `VReq`, `VOut`, `VFbk`, `VType`
- `InputElement` (SUC): `WQC`, `VQC`
- `AnaProcessValueIn` (SUC): `OSLevel`, `V`, `UnitCur`, `SclMinCur`, `SclMaxCur`, `UnitExt`, `SclMinExt`, `SclMaxExt`, `Apply`, `ApplyEn`
- `DIntProcessValueIn` (SUC): `OSLevel`, `V`, `UnitCur`, `SclMinCur`, `SclMaxCur`, `UnitExt`, `SclMinExt`, `SclMaxExt`, `Apply`, `ApplyEn`
- `BinProcessValueIn` (SUC): `V`, `VState0`, `VState1`
- `StringProcessValueIn` (SUC): `V`
- `StructProcessValueIn` (SUC): `V`, `VType`
- `ArrayProcessValueIn` (SUC): `IndexSel`, `IndexMin`, `IndexMax`, `IndexCur`, `V`, `VType`
- `OutputElement` (SUC): `WQC`
- `ArrayProcessValueOut` (SUC): `IndexSel`, `IndexMin`, `IndexMax`, `IndexCur`, `V`, `VType`

### Model object types
- `ServiceSet` (SUC): root of Service definitions under an aspect entry.
- `Service` (SUC): child of ServiceSet; key attribute `Classification`; children: Procedures + ConfigurationParameters.
- `Procedure` (SUC, ServiceSet.Base): `IsSelfCompleting`, `ProcedureID`, `Classification`; children: ProcedureParameters, ReportValues, RequiredEquipment.
- `ServiceParameter` (SUC): base parameter model object; key attribute `Classification`.
- `ProcedureParameter` (SUC): specialization of ServiceParameter.
- `ConfigurationParameter` (SUC): specialization of ServiceParameter.
- `ReportValue` (SUC): linked report output model object.
- `RequiredEquipment` (SUC): linked equipment dependency model object.
- `HasServiceInteraction` (RC), `ServiceInteraction` (SUC), `Question` (SUC), `Answer` (SUC)
- `HasServicePosition` (RC), `ServicePosition` (SUC), `PositionText` (SUC)
- `ProcessValues` (IH)
- `ProcessValueSet` (SUC)
- `ProcessValue` (SUC abstract), `ProcessValueIn` (SUC), `ProcessValueOut` (SUC)
- `Procedure` (ProcessValue profile extension): same procedure core with added `ProcessValue` children.

### Attribute-type object types
- `FunctionClassificationAttribute` (AT): `Standard`, `Level`, `Type`, `IRDI`
- `InputElementConfigType` (AT): ordered key/value-list style config records

## Derived Underhill MTP-PEA data model (3 subsystems)

## 1) Composition model
- `underhill-base` (BaseEquipment)
  - `AIRLOCK-PEA-001` (PEAType)
    - `AIRLOCK-PEA-001:AirlockService` (ServiceType)
  - `ECLSS-PEA-001` (PEAType)
    - `ECLSS-PEA-001:EclssService` (ServiceType)
  - `SABATIER-PEA-001` (PEAType)
    - `SABATIER-PEA-001:SabatierService` (ServiceType)

## 2) PEA descriptors

### Airlock PEA
- `pea_id`: `AIRLOCK-PEA-001`
- `pea_type`: `AIRLOCK`
- `name`: `Underhill Airlock`
- `namespace_uri`: `urn:mars-airlock:mtp`
- `root_path`: `Objects/MarsBase/AirlockPEA`
- `endpoint`: `opc.tcp://127.0.0.1:4841/underhill/airlock` (default)
- `service_tag`: `AirlockService`
- runtime/lifecycle keys: `deployed`, `running`, `last_transition_ms`
- service keys: `state`, `transition_active`, `active_procedure`, `command_en`

### ECLSS PEA
- `pea_id`: `ECLSS-PEA-001`
- `pea_type`: `ECLSS`
- `name`: `Underhill ECLSS`
- `namespace_uri`: `urn:underhill:{node_id}:pea:ECLSS-PEA-001`
- `root_path`: `Objects/Underhill/ECLSSPEA`
- `endpoint`: `opc.tcp://127.0.0.1:4842/underhill/eclss` (default)
- `service_tag`: `EclssService`
- runtime/lifecycle keys: `deployed`, `running`, `last_transition_ms`
- operator-state keys: `operation_mode`, `source_mode`, `command_en`, `command_en_reason`, `operator_control_enabled`, `remote_control_enabled`
- process keys: `cabin_pressure_kpa`, `o2_percent`, `co2_ppm`, `humidity_pct`, `water_recovery_pct`, `co2_capture_kgph`, `o2_generation_kgph`, `power_kw`, `alarm_high_co2`, `alarm_low_o2`

### Sabatier PEA
- `pea_id`: `SABATIER-PEA-001`
- `pea_type`: `ISRU_SABATIER`
- `name`: `Underhill Sabatier`
- `namespace_uri`: `urn:underhill:{node_id}:pea:SABATIER-PEA-001`
- `root_path`: `Objects/Underhill/SabatierPEA`
- `endpoint`: `opc.tcp://127.0.0.1:4843/underhill/sabatier` (default)
- `service_tag`: `SabatierService`
- runtime/lifecycle keys: `deployed`, `running`, `last_transition_ms`
- operator-state keys: same schema as ECLSS
- process keys: `reactor_temp_c`, `reactor_pressure_bar`, `co2_feed_kgph`, `h2_feed_kgph`, `conversion_efficiency_pct`, `methane_production_kgph`, `water_production_kgph`, `catalyst_health_pct`, `power_kw`, `alarm_reactor_temp`

## 3) Service + procedure objects implemented

### AirlockService
- Procedures implemented:
  - `Proc_DepressurizeForEVA` (ID 1)
  - `Proc_PressurizeForEntry` (ID 2)
  - `Proc_ManualDoorJog` (ID 3)
- Procedure runtime keys per procedure:
  - `State`
  - `ProgressPct`
  - `LastResult`
- Procedure parameter keys (current implementation):
  - Depressurize: `TargetPressurePa`
  - Pressurize: `TargetPressurePa`
  - Manual jog: `TargetDoorPct`

### EclssService
- Procedure modeling currently nominal/derived for descriptors (`Proc_LifeSupportNominal` / `Proc_Nominal` semantics), with no full procedure command path exposed.
- ServiceControl/state is represented in OPC UA by:
  - `State`, `Deployed`, `Running`

### SabatierService
- Procedure modeling currently nominal/derived for descriptors (`Proc_SabatierNominal` / `Proc_Nominal` semantics), with no full procedure command path exposed.
- ServiceControl/state represented by:
  - `State`, `Deployed`, `Running`

## 4) Control-command handshake model (implemented)

Implemented under Airlock Control branches:
- `OperatorCommands/Req`: `SequenceId`, `Command`, `Param1`, `Param2`, `Execute`
- `OperatorCommands/Rsp`: `AckSequenceId`, `Status`, `RejectReason`, `LastUpdateTimeMs`
- `RemoteCommands/Req`: same schema as operator
- `RemoteCommands/Rsp`: same schema as operator

Active command object:
- `Command`, `Source`, `SequenceId`, `Param1`, `Param2`, `State`, `ProgressPct`, `BlockingCondition`, `StartTimeMs`, `LastUpdateTimeMs`

Command ID map:
- `0 None`, `1 StartDepressurizeCycle`, `2 StartPressurizeCycle`, `3 AbortCycle`, `4 ResetFaults`, `5 SetPumpOn`, `6 SetEqualizeValvePct`, `7 SetVentValvePct`, `8 SetInnerDoorTargetPct`, `9 SetOuterDoorTargetPct`, `10 LockInnerDoor`, `11 UnlockInnerDoor`, `12 LockOuterDoor`, `13 UnlockOuterDoor`

Command status ID map:
- `0 Idle`, `1 Accepted`, `2 Rejected`, `3 Running`, `4 Complete`, `5 Aborted`

## 5) State-code alignment

Current code aligns Airlock state codes to Part 4 Table 3 values:
- `Idle=16`, `Starting=8`, `Execute=64`, `Completing=65536`, `Completed=131072`, `Pausing=8192`, `Paused=32`, `Resuming=16384`, `Holding=1024`, `Held=2048`, `Unholding=4096`, `Stopping=128`, `Stopped=4`, `Aborting=256`, `Aborted=512`, `Resetting=32768`

Subsystem (ECLSS/Sabatier) simplified derived map in runtime API:
- `Execute=64`, `Held=2048`, `Stopped=4`, default `Idle=16`

## 6) Practical “key-value object schema” for the three subsystem PEAs

```yaml
UnderhillBase:
  peas:
    - pea_id: AIRLOCK-PEA-001
      service:
        tag: AirlockService
        procedures:
          - { id: 1, name: Proc_DepressurizeForEVA }
          - { id: 2, name: Proc_PressurizeForEntry }
          - { id: 3, name: Proc_ManualDoorJog }
      keys:
        diagnostics: [EndpointUrl, SecurityMode, ServerStartTimeMs, ServerUptimeSec, ConnectedClientCount, ConnectedClientSummary, SubscriptionCount, PublishingRateHz, LastRejectedCommand, LastError]
        modes: [OperationMode, SourceMode, CommandEn, CommandEnReason, ApplyEn, ApplyEnReason]
        state_machine: [CurrentState, TimeInStateSec, TransitionActive, BlockingCondition]
        indicators: [PressurePa, TemperatureK, O2Percent, InnerDoorPositionPct, OuterDoorPositionPct, InnerDoorClosed, OuterDoorClosed, PumpOn, EqualizeValvePct, VentValvePct, AlarmSummary, OutOfSpec]
        parameters: [OperatorControlEnabled, RemoteControlEnabled, LeakRateNominal, OuterUnlockMaxPressurePa, InnerUnlockMinPressurePa, HighPressureAlarmPa, LowPressureAlarmPa]
        control: [OperatorCommands.Req/Rsp, RemoteCommands.Req/Rsp, ActiveCommand]

    - pea_id: ECLSS-PEA-001
      service:
        tag: EclssService
      keys:
        service_control: [State, Deployed, Running]
        indicators: [O2Percent, CO2ppm, HumidityPct, WaterRecoveryPct, PowerKw, AlarmHighCO2, AlarmLowO2]
        operator_state: [operation_mode, source_mode, command_en, command_en_reason, operator_control_enabled, remote_control_enabled]

    - pea_id: SABATIER-PEA-001
      service:
        tag: SabatierService
      keys:
        service_control: [State, Deployed, Running]
        indicators: [ReactorTempC, ReactorPressureBar, MethaneProductionKgph, WaterProductionKgph, ConversionEfficiencyPct, CatalystHealthPct, PowerKw, AlarmReactorTemp]
        operator_state: [operation_mode, source_mode, command_en, command_en_reason, operator_control_enabled, remote_control_enabled]
```
