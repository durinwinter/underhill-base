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

Initial manifests cover DAMADICS, ISDB, the SACAC PID repository, and the SWaT/WADI/BATADAL security family. DAMADICS supplies actuator fault signatures; ISDB focuses stiction behavior; SACAC supplies broader poor-loop root-cause labels; the security family defines held-out cyber-physical campaign shapes. None authorizes direct copying of terrestrial process magnitudes or protocol claims into the Mars base.

The live runner currently provides `airlock_equalize_stiction` and
`safety_compound_leak_fire` templates. These are deterministic native Underhill campaigns tied to
manifest IDs, not replays of raw source records. They run against the continuing plant clock,
restore their bounded injection inputs on completion, retain physical consequences, and produce a
durable diagnosis/latency/confidence report.
