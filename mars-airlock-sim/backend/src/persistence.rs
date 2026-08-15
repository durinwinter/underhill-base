use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    PeaRuntimeState, SubsystemOperatorState,
    plant_runtime::PlantSchedulerState,
    sim::Simulation,
    subsystems::{EclssSimulation, SabatierSimulation},
};

pub const CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const JOURNAL_SCHEMA_VERSION: u32 = 1;

const DEFAULT_PLANT_ID: &str = "underhill-base-primary";
const DEFAULT_CHECKPOINT_INTERVAL_SEC: f64 = 60.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantCheckpoint {
    pub schema_version: u32,
    pub plant_id: String,
    pub saved_wall_time_ms: u64,
    pub scheduler: PlantSchedulerState,
    pub airlock: Simulation,
    pub eclss: EclssSimulation,
    pub sabatier: SabatierSimulation,
    pub airlock_runtime: PeaRuntimeState,
    pub eclss_runtime: PeaRuntimeState,
    pub sabatier_runtime: PeaRuntimeState,
    pub eclss_operator_state: SubsystemOperatorState,
    pub sabatier_operator_state: SubsystemOperatorState,
}

impl PlantCheckpoint {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        plant_id: String,
        scheduler: PlantSchedulerState,
        airlock: Simulation,
        eclss: EclssSimulation,
        sabatier: SabatierSimulation,
        airlock_runtime: PeaRuntimeState,
        eclss_runtime: PeaRuntimeState,
        sabatier_runtime: PeaRuntimeState,
        eclss_operator_state: SubsystemOperatorState,
        sabatier_operator_state: SubsystemOperatorState,
    ) -> Self {
        Self {
            schema_version: CHECKPOINT_SCHEMA_VERSION,
            plant_id,
            saved_wall_time_ms: wall_time_ms(),
            scheduler,
            airlock,
            eclss,
            sabatier,
            airlock_runtime,
            eclss_runtime,
            sabatier_runtime,
            eclss_operator_state,
            sabatier_operator_state,
        }
    }

    fn validate(&self, expected_plant_id: &str) -> Result<()> {
        if self.schema_version != CHECKPOINT_SCHEMA_VERSION {
            bail!(
                "unsupported checkpoint schema version {}; expected {}",
                self.schema_version,
                CHECKPOINT_SCHEMA_VERSION
            );
        }
        if self.plant_id != expected_plant_id {
            bail!(
                "checkpoint plant_id {} does not match configured plant_id {}",
                self.plant_id,
                expected_plant_id
            );
        }
        if !self.scheduler.plant_elapsed_sec.is_finite() || self.scheduler.plant_elapsed_sec < 0.0 {
            bail!("checkpoint plant elapsed time is invalid");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalRecord {
    pub schema_version: u32,
    pub sequence: u64,
    pub wall_time_ms: u64,
    pub plant_elapsed_sec: f64,
    pub kind: String,
    pub subject: String,
    pub payload: Value,
}

pub struct PlantPersistence {
    plant_id: String,
    state_dir: PathBuf,
    checkpoint_path: PathBuf,
    previous_checkpoint_path: PathBuf,
    journal_path: PathBuf,
    checkpoint_interval_sec: f64,
    journal_sequence: Mutex<u64>,
}

impl PlantPersistence {
    pub fn from_env() -> Result<Self> {
        let state_dir = env::var("UNDERHILL_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("data")
                    .join("continuous")
            });
        let plant_id =
            env::var("UNDERHILL_PLANT_ID").unwrap_or_else(|_| DEFAULT_PLANT_ID.to_string());
        let checkpoint_interval_sec = env::var("UNDERHILL_CHECKPOINT_INTERVAL_SEC")
            .ok()
            .map(|value| {
                value.parse::<f64>().with_context(|| {
                    format!(
                        "Invalid UNDERHILL_CHECKPOINT_INTERVAL_SEC value {value}; expected a number"
                    )
                })
            })
            .transpose()?
            .unwrap_or(DEFAULT_CHECKPOINT_INTERVAL_SEC);
        Self::new(state_dir, plant_id, checkpoint_interval_sec)
    }

    pub fn new(state_dir: PathBuf, plant_id: String, checkpoint_interval_sec: f64) -> Result<Self> {
        if plant_id.trim().is_empty() {
            bail!("UNDERHILL_PLANT_ID must not be empty");
        }
        if !checkpoint_interval_sec.is_finite() || checkpoint_interval_sec <= 0.0 {
            bail!("checkpoint interval must be finite and greater than zero");
        }
        fs::create_dir_all(&state_dir).with_context(|| {
            format!(
                "failed to create plant state directory {}",
                state_dir.display()
            )
        })?;
        let checkpoint_path = state_dir.join("plant-checkpoint.json");
        let previous_checkpoint_path = state_dir.join("plant-checkpoint.previous.json");
        let journal_path = state_dir.join("plant-events.ndjson");
        let journal_sequence = read_last_journal_sequence(&journal_path)?;
        Ok(Self {
            plant_id,
            state_dir,
            checkpoint_path,
            previous_checkpoint_path,
            journal_path,
            checkpoint_interval_sec,
            journal_sequence: Mutex::new(journal_sequence),
        })
    }

    pub fn plant_id(&self) -> &str {
        &self.plant_id
    }

    pub fn checkpoint_interval_sec(&self) -> f64 {
        self.checkpoint_interval_sec
    }

    pub fn load_checkpoint(&self) -> Result<Option<PlantCheckpoint>> {
        match read_checkpoint(&self.checkpoint_path, &self.plant_id) {
            Ok(Some(checkpoint)) => Ok(Some(checkpoint)),
            Ok(None) => read_checkpoint(&self.previous_checkpoint_path, &self.plant_id),
            Err(primary_error) => {
                match read_checkpoint(&self.previous_checkpoint_path, &self.plant_id) {
                    Ok(Some(checkpoint)) => Ok(Some(checkpoint)),
                    Ok(None) => Err(primary_error).context("primary checkpoint is invalid and no previous checkpoint exists"),
                    Err(previous_error) => Err(primary_error).context(format!(
                        "primary and previous checkpoints are invalid; previous error: {previous_error:#}"
                    )),
                }
            }
        }
    }

    pub fn save_checkpoint(&self, checkpoint: &PlantCheckpoint) -> Result<()> {
        checkpoint.validate(&self.plant_id)?;
        let payload = serde_json::to_vec_pretty(checkpoint)
            .context("failed to serialize full plant checkpoint")?;
        let temporary_path = self.state_dir.join("plant-checkpoint.tmp");

        let mut temporary = File::create(&temporary_path)
            .with_context(|| format!("failed to create {}", temporary_path.display()))?;
        temporary
            .write_all(&payload)
            .with_context(|| format!("failed to write {}", temporary_path.display()))?;
        temporary
            .sync_all()
            .with_context(|| format!("failed to sync {}", temporary_path.display()))?;

        if self.checkpoint_path.exists() {
            fs::copy(&self.checkpoint_path, &self.previous_checkpoint_path).with_context(|| {
                format!(
                    "failed to preserve previous checkpoint at {}",
                    self.previous_checkpoint_path.display()
                )
            })?;
        }
        fs::rename(&temporary_path, &self.checkpoint_path).with_context(|| {
            format!(
                "failed to atomically install checkpoint {}",
                self.checkpoint_path.display()
            )
        })?;
        sync_directory(&self.state_dir)?;
        Ok(())
    }

    pub fn append_journal(
        &self,
        plant_elapsed_sec: f64,
        kind: impl Into<String>,
        subject: impl Into<String>,
        payload: Value,
    ) -> Result<JournalRecord> {
        if !plant_elapsed_sec.is_finite() || plant_elapsed_sec < 0.0 {
            bail!("journal plant elapsed time must be finite and non-negative");
        }
        let mut sequence = self
            .journal_sequence
            .lock()
            .map_err(|_| anyhow::anyhow!("journal sequence mutex poisoned"))?;
        *sequence = sequence
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("journal sequence exhausted"))?;
        let record = JournalRecord {
            schema_version: JOURNAL_SCHEMA_VERSION,
            sequence: *sequence,
            wall_time_ms: wall_time_ms(),
            plant_elapsed_sec,
            kind: kind.into(),
            subject: subject.into(),
            payload,
        };
        let mut journal = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal_path)
            .with_context(|| format!("failed to open {}", self.journal_path.display()))?;
        serde_json::to_writer(&mut journal, &record)
            .context("failed to serialize journal record")?;
        journal
            .write_all(b"\n")
            .context("failed to terminate journal record")?;
        journal
            .sync_data()
            .context("failed to sync operational journal")?;
        Ok(record)
    }
}

fn read_checkpoint(path: &Path, expected_plant_id: &str) -> Result<Option<PlantCheckpoint>> {
    let payload = match fs::read(path) {
        Ok(payload) => payload,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", path.display())),
    };
    let checkpoint: PlantCheckpoint = serde_json::from_slice(&payload)
        .with_context(|| format!("failed to parse checkpoint {}", path.display()))?;
    checkpoint.validate(expected_plant_id)?;
    Ok(Some(checkpoint))
}

fn read_last_journal_sequence(path: &Path) -> Result<u64> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", path.display())),
    };
    let mut last_sequence = 0;
    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line =
            line.with_context(|| format!("failed reading journal line {}", line_index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: JournalRecord = serde_json::from_str(&line)
            .with_context(|| format!("invalid journal record at line {}", line_index + 1))?;
        if record.schema_version != JOURNAL_SCHEMA_VERSION {
            bail!(
                "unsupported journal schema version {} at line {}",
                record.schema_version,
                line_index + 1
            );
        }
        if record.sequence <= last_sequence {
            bail!(
                "journal sequence is not strictly increasing at line {}",
                line_index + 1
            );
        }
        last_sequence = record.sequence;
    }
    Ok(last_sequence)
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .with_context(|| format!("failed to open state directory {}", path.display()))?
        .sync_all()
        .with_context(|| format!("failed to sync state directory {}", path.display()))
}

pub fn wall_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plant_runtime::PlantSchedulerState,
        sim::Simulation,
        subsystems::{EclssSimulation, SabatierSimulation},
    };

    fn temp_state_dir(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "underhill-{test_name}-{}-{}",
            std::process::id(),
            wall_time_ms()
        ))
    }

    fn sample_checkpoint(plant_id: &str, elapsed: f64) -> PlantCheckpoint {
        PlantCheckpoint::new(
            plant_id.to_string(),
            PlantSchedulerState {
                accumulated_sim_sec: 0.0,
                plant_elapsed_sec: elapsed,
                step_index: (elapsed / 0.05) as u64,
            },
            Simulation::new("NONE".to_string(), "opc.tcp://test".to_string()),
            EclssSimulation::new(),
            SabatierSimulation::new(),
            PeaRuntimeState {
                deployed: true,
                running: true,
                last_transition_ms: 1,
            },
            PeaRuntimeState {
                deployed: true,
                running: true,
                last_transition_ms: 2,
            },
            PeaRuntimeState {
                deployed: true,
                running: false,
                last_transition_ms: 3,
            },
            SubsystemOperatorState::default(),
            SubsystemOperatorState::default(),
        )
    }

    #[test]
    fn checkpoint_round_trip_preserves_full_model_state() {
        let state_dir = temp_state_dir("checkpoint-round-trip");
        let persistence =
            PlantPersistence::new(state_dir.clone(), "test-plant".to_string(), 60.0).unwrap();
        persistence
            .save_checkpoint(&sample_checkpoint("test-plant", 123.45))
            .unwrap();
        let loaded = persistence.load_checkpoint().unwrap().unwrap();
        assert!((loaded.scheduler.plant_elapsed_sec - 123.45).abs() < 1.0e-12);
        assert!(loaded.airlock_runtime.running);
        assert!(!loaded.sabatier_runtime.running);
        assert!((loaded.eclss.snapshot().co2_ppm - 950.0).abs() < 1.0e-12);
        fs::remove_dir_all(state_dir).unwrap();
    }

    #[test]
    fn previous_checkpoint_recovers_from_corrupt_primary() {
        let state_dir = temp_state_dir("checkpoint-recovery");
        let persistence =
            PlantPersistence::new(state_dir.clone(), "test-plant".to_string(), 60.0).unwrap();
        persistence
            .save_checkpoint(&sample_checkpoint("test-plant", 10.0))
            .unwrap();
        persistence
            .save_checkpoint(&sample_checkpoint("test-plant", 20.0))
            .unwrap();
        fs::write(&persistence.checkpoint_path, b"not-json").unwrap();
        let recovered = persistence.load_checkpoint().unwrap().unwrap();
        assert!((recovered.scheduler.plant_elapsed_sec - 10.0).abs() < 1.0e-12);
        fs::remove_dir_all(state_dir).unwrap();
    }

    #[test]
    fn journal_sequence_continues_after_reopen() {
        let state_dir = temp_state_dir("journal-sequence");
        let persistence =
            PlantPersistence::new(state_dir.clone(), "test-plant".to_string(), 60.0).unwrap();
        let first = persistence
            .append_journal(1.0, "test", "first", serde_json::json!({}))
            .unwrap();
        assert_eq!(first.sequence, 1);
        drop(persistence);
        let reopened =
            PlantPersistence::new(state_dir.clone(), "test-plant".to_string(), 60.0).unwrap();
        let second = reopened
            .append_journal(2.0, "test", "second", serde_json::json!({}))
            .unwrap();
        assert_eq!(second.sequence, 2);
        fs::remove_dir_all(state_dir).unwrap();
    }
}
