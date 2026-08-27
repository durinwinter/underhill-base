# Empirical Dataset Manifests

These manifests define how external evidence may be used without confusing a terrestrial process dataset with Mars hardware truth. They are intentionally metadata-only: raw data is not downloaded or committed until source terms, file inventory, and SHA-256 checksums are recorded.

The backend test suite parses every JSON file in `manifests/` and rejects unsupported schema versions, incomplete provenance/license records, missing physical-domain gaps, non-reproducible partitions, incomplete signal mappings, or malformed acceptance thresholds.

## Required ingestion sequence

1. Verify the publisher landing page and record the retrieval date.
2. Review redistribution and permitted-use terms; update `license.status` and `license.identifier`.
3. Download outside Git, inventory every selected artifact, and record SHA-256 checksums.
4. Implement a deterministic transformation identified by a versioned name.
5. Assign whole runs/loops—not individual rows—to the declared calibration and held-out partitions.
6. Fit only on the calibration/development partition.
7. Score the unchanged Underhill profile and diagnostic layer on the held-out partition.
8. Store the manifest ID, transformation version, partition selector, model/profile version, and score report with the validation run.

`source_supplied` means the project has a user-provided canonical-looking location but has not yet completed independent terms/file verification. `source_verified` means the publisher/repository provenance is verified; it does not mean raw-file licensing or integrity checks are complete.

Initial manifests cover DAMADICS, ISDB, the SACAC PID repository, the SWaT/WADI/BATADAL security family, and NASA ICES-2025-127. DAMADICS supplies actuator fault signatures; ISDB focuses stiction behavior; SACAC supplies broader poor-loop root-cause labels; the security family defines held-out cyber-physical campaign shapes; ICES-2025-127 supplies published MADS-derived ECLSS aggregate reliability fields. None authorizes direct copying of terrestrial process magnitudes or unsupported flight-data claims into the Mars base.

The live runner currently provides `airlock_equalize_stiction`,
`safety_compound_leak_fire`, `maintenance_shared_tool_contention`, and
`water_conductivity_replay` templates. These are
deterministic native Underhill campaigns tied to manifest IDs, not replays of raw source records.
They run against the continuing plant clock, restore or remove their bounded injection inputs on
completion, retain legitimate physical consequences, and produce a durable
diagnosis/latency/confidence report. The maintenance campaign injects two synthetic work demands
that compete for one water-loop service kit; it exercises the real persistent priority queue,
resource reservations, historian quality, and agent observation API without consuming a flight
spare or altering ECLSS chemistry.

The Airlock stiction campaign now performs a seed-varying, bounded three-stage valve stimulus,
records commanded/true/sensed position, residual, stiction state, and pressure at one-second plant
boundaries, and emits a durable trace-qualification report. Active campaign discovery is blinded:
template identity, dataset identity, seed, ground truth, baselines, trace values, and other agents'
answers are withheld until completion. Completed traces are retrieved through the paginated
`/api/v1/validation/campaigns/{campaign_id}/trace` endpoint rather than expanding every campaign
summary. The qualification thresholds are versioned in
`contracts/underhill-airlock-stiction-trace-v1.json`.

That contract is deliberately labeled `underhill_native_damadics_informed`. The official DAMADICS
landing page and data inventory have been verified, but the page does not publish sufficiently clear
file-level redistribution terms. Until terms, selected archive checksums, deterministic transforms,
and held-out run identities are recorded, Underhill must not describe this result as raw-data replay,
DAMADICS calibration, or held-out DAMADICS comparison.

`water_conductivity_replay` injects a seed-varying physical conductivity target while the public
sensor and its derived alarm replay their nominal baseline across every supported process-data
surface. The protected one-second trace records true and observed values and alarms, then scores
true excursion, observed flatness, peak divergence, and concealed-alarm duration using
`contracts/underhill-water-conductivity-replay-v1.json`. Its evidence class is
`underhill_native_swat_wadi_batadal_informed`: it validates Underhill's native telemetry-integrity
path and does not claim source-protocol or raw-dataset replay.

`airlock_command_sequence_replay` exercises the command handler shared by REST and OPC UA. A safe
accepted command establishes the remote sequence watermark; a conflicting duplicate and an older
outer-door-unlock attempt must then be rejected without moving that watermark or changing door and
lock state. The completed protected trace is scored by
`contracts/underhill-airlock-command-replay-v1.json`. Its evidence class is also
`underhill_native_swat_wadi_batadal_informed`; it is an Underhill-native command-path test, not a
replay of source packets or protocol traffic.
