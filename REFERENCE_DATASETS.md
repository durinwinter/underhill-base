# Underhill Reference Dataset Catalog

## Purpose

Underhill uses external datasets to calibrate model behavior and to validate agents, controllers, diagnostics, maintenance predictors, and SCADA-security monitors. A reference dataset is never copied blindly into the simulated Mars base. Its role, provenance, license, units, operating envelope, uncertainty, and domain mismatch must be recorded before derived parameters or fixtures enter a released model.

Each approved dataset receives a versioned manifest containing:

- canonical name, publisher, source URL, retrieval date, checksum, and license;
- raw/processed file inventory and an immutable transformation recipe;
- represented equipment, sensors, actuators, sampling rates, labels, and operating regimes;
- Underhill PEA/tag mappings and unit conversions;
- whether it calibrates physics, supplies a fault signature, validates an algorithm, or is only an architectural analog;
- exclusions, domain gaps, uncertainty, and allowed claims;
- train/calibration/validation partitions that prevent an agent being evaluated on its training fixture;
- tests for schema, ranges, missing values, label integrity, and deterministic replay.

Raw third-party data does not enter Git unless its license and size make that appropriate. Git stores manifests, download instructions, checksums, transformations, compact derived fixtures where permitted, and provenance notes.

## Priority A: direct control-loop and actuator evidence

### DAMADICS

- **Underhill use:** Airlock equalization and vent valves initially; later ECLSS gas and water valves.
- **Role:** Calibrate and validate actuator/sensor fault signatures such as bias, stiction, leakage, positioner faults, and abnormal valve response.
- **Mapped model elements:** commanded position, true stem position, sensed position, pressure differential, mass flow, actuator effort, travel time, deadband, interlock response, and residuals.
- **Validation gate:** an injected Underhill valve fault must reproduce the qualitative signature and comparable normalized residual/response metrics without copying unrelated chemical-process conditions.
- **Source supplied:** [DAMADICS benchmark](https://iair.mchtr.pw.edu.pl/Damadics).

### International Stiction Database (ISDB)

- **Underhill use:** Dynamic friction, deadband, stick-slip, and hysteresis models for Airlock, ECLSS, water, thermal, and ISRU control valves.
- **Role:** Parameter calibration and held-out loop-response validation.
- **Important limitation:** Process conditions and valve hardware must be mapped explicitly; recorded output shapes are evidence, not universal constants.

### SACAC control-loop datasets

- **Underhill use:** Labeled diagnostic fixtures for oscillation, valve stiction, poor tuning, and external disturbance.
- **Role:** Evaluate root-cause classification and control-loop health agents independently from the simulated training campaigns.
- **Acceptance metrics:** detection latency, false alarms, correct root cause, confidence calibration, and safe recommendation quality.

## Priority B: structural process-simulator references

### Tennessee Eastman Process (TEP)

- **Underhill use:** Architectural reference for coupled unit operations, multivariable control, fault campaigns, and controller/plant separation.
- **Role:** Design analog only; its chemistry and nominal parameters do not calibrate Mars ECLSS or Sabatier physics.
- **Applicable lessons:** deterministic fault scheduling, interacting loops, controller observability, benchmark splits, and cascade evaluation.

### IndPenSim

- **Underhill use:** Reference for a physics-based process simulator that emits control-loop-shaped telemetry with realistic measurement and control layers.
- **Role:** Structural analog for Sabatier and future bioreactor/agriculture PEAs, not a reaction-data source for Sabatier chemistry.

### Naval propulsion condition-based-maintenance data

- **Underhill use:** Pattern for labeled degradation trajectories in coupled thermal/mechanical equipment.
- **Role:** Method reference for generating remaining-useful-life and maintenance evidence for pumps, compressors, blowers, turbomachinery, and thermal loops.
- **Restriction:** Naval plant parameters are not treated as Mars hardware parameters.

## Priority C: SCADA process-security validation

### SWaT, WADI, and BATADAL

- **Underhill use:** Water-recovery structural analogs and, more importantly, held-out attack/anomaly patterns for the WinCC OA and OPC UA validation surface.
- **Role:** Shape cyber-physical campaigns involving spoofed measurements, malicious setpoints, replay, stale values, denial/loss of view, unsafe sequencing, and coordinated multistage attacks.
- **Translation rule:** Dataset protocol details are not assumed to be OPC UA. Campaigns are re-expressed through Underhill's authority, command-handshake, telemetry-quality, and network-fault models.
- **Evaluation:** distinguish equipment faults, sensor faults, operator actions, controller defects, and malicious behavior while preserving deterministic safety interlocks.
- **Security rule:** attack corpora are isolated from the canonical live plant and executed in checkpoint-derived validation forks unless an explicitly authorized live drill is active.

## Priority D: power, thermal, and machinery degradation

### NASA and other traceable Li-ion battery aging datasets

- **Underhill use:** Battery capacity fade, resistance growth, temperature dependence, cycling damage, state-of-health estimation, and remaining-useful-life validation.
- **Role:** Calibrate the future Power PEA only after cell chemistry, format, duty cycle, thermal environment, and extrapolation limits are matched.
- **Mars-specific extension:** couple measured cell behavior to modeled pack topology, radiation uncertainty, dust-driven solar variability, thermal-control limitations, and islanded DC-bus operation.

### FEMTO/PRONOSTIA and CWRU bearing datasets

- **Underhill use:** Pump, fan, blower, compressor, centrifuge, and robotic-joint condition monitoring.
- **Role:** Vibration-feature and degradation-detection fixtures; not direct lifetime calibration for unmatched Mars-rated bearings.
- **Model mapping:** rotational speed, load, temperature, vibration waveform/features, lubrication state, health estimate, alarm confidence, and maintenance decision.

## Priority E: ECLSS reliability

### ICES-2025-127 and ISS MADS-derived publications

- **Underhill use:** Candidate reliability and maintainability evidence for OGS, CDRA, UPA, WPA, and Sabatier/CRS components.
- **Role:** Failure/repair distribution calibration where the publication exposes sufficient population, exposure, censoring, and uncertainty information.
- **Restriction:** published aggregate results are not described as the raw ISS Maintenance and Analysis Data Set. Future Mars-base performance remains configurable and uncertainty-bounded.

## Explicit exclusions

The following are excluded from physical calibration because keyword similarity does not overcome the physics mismatch:

- combined-cycle gas-turbine power-plant datasets;
- terrestrial wind-turbine SCADA datasets;
- bulk AC-grid stability datasets;
- household/appliance electricity datasets such as UK-DALE, ECO, and GREEND.

They may be reconsidered only for a narrowly documented algorithmic test that does not claim Mars-base physical fidelity.

## Implementation order

1. Create the manifest schema and checksum/download verifier.
2. Ingest DAMADICS metadata and define the Airlock valve tag mapping.
3. Replace ideal valve motion with a parameterized actuator model supporting lag, rate limit, deadband, hysteresis, stiction, leakage, bias, noise, and failure modes.
4. Keep a held-out DAMADICS/ISDB/SACAC validation partition and publish comparison metrics.
5. Build SWaT/WADI/BATADAL-inspired campaigns through Underhill's native OPC UA/authority model.
6. Add ECLSS reliability and battery/bearing degradation manifests as their PEAs become physically modeled.
7. Record every dataset-derived parameter set and campaign identifier in checkpoints, historian metadata, and evaluation reports.

