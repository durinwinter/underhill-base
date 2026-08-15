use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const HISTORIAN_SCHEMA_VERSION: u32 = 1;
const DEFAULT_RECENT_CAPACITY: usize = 50_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TelemetryQuality {
    Good,
    Uncertain,
    Stale,
    Bad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorianSample {
    pub schema_version: u32,
    pub sequence: u64,
    pub tag_id: String,
    pub wall_time_ms: u64,
    pub plant_elapsed_sec: f64,
    pub value: Value,
    pub quality: TelemetryQuality,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct NewHistorianSample {
    pub tag_id: String,
    pub value: Value,
    pub quality: TelemetryQuality,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistorianStats {
    pub schema_version: u32,
    pub archive_path: String,
    pub durable_records: u64,
    pub recent_records: usize,
    pub recent_capacity: usize,
    pub first_recent_sequence: Option<u64>,
    pub last_sequence: u64,
}

struct HistorianInner {
    file: File,
    recent: VecDeque<HistorianSample>,
    last_sequence: u64,
}

pub struct Historian {
    archive_path: PathBuf,
    recent_capacity: usize,
    inner: Mutex<HistorianInner>,
}

impl Historian {
    pub fn open(state_dir: &Path) -> Result<Self> {
        let recent_capacity = std::env::var("UNDERHILL_HISTORIAN_RECENT_CAPACITY")
            .ok()
            .map(|value| {
                value
                    .parse::<usize>()
                    .with_context(|| format!("invalid UNDERHILL_HISTORIAN_RECENT_CAPACITY {value}"))
            })
            .transpose()?
            .unwrap_or(DEFAULT_RECENT_CAPACITY);
        if recent_capacity == 0 {
            bail!("UNDERHILL_HISTORIAN_RECENT_CAPACITY must be greater than zero");
        }
        Self::open_with_capacity(state_dir, recent_capacity)
    }

    fn open_with_capacity(state_dir: &Path, recent_capacity: usize) -> Result<Self> {
        fs::create_dir_all(state_dir).with_context(|| {
            format!(
                "failed to create historian directory {}",
                state_dir.display()
            )
        })?;
        let archive_path = state_dir.join("telemetry-history.ndjson");
        let (recent, last_sequence) = recover_archive(&archive_path, recent_capacity)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&archive_path)
            .with_context(|| {
                format!(
                    "failed to open historian archive {}",
                    archive_path.display()
                )
            })?;
        Ok(Self {
            archive_path,
            recent_capacity,
            inner: Mutex::new(HistorianInner {
                file,
                recent,
                last_sequence,
            }),
        })
    }

    #[cfg(test)]
    pub fn append_batch(
        &self,
        wall_time_ms: u64,
        plant_elapsed_sec: f64,
        samples: Vec<NewHistorianSample>,
    ) -> Result<usize> {
        self.append_frames(wall_time_ms, vec![(plant_elapsed_sec, samples)])
    }

    pub fn append_frames(
        &self,
        wall_time_ms: u64,
        frames: Vec<(f64, Vec<NewHistorianSample>)>,
    ) -> Result<usize> {
        let mut inner = self.inner.lock().expect("historian mutex poisoned");
        let mut encoded = Vec::new();
        let record_capacity = frames.iter().map(|(_, samples)| samples.len()).sum();
        let mut records = Vec::with_capacity(record_capacity);
        let opening_sequence = inner.last_sequence;
        for (plant_elapsed_sec, samples) in frames {
            if !plant_elapsed_sec.is_finite() || plant_elapsed_sec < 0.0 {
                bail!("historian plant time must be finite and non-negative");
            }
            for sample in samples {
                let record = HistorianSample {
                    schema_version: HISTORIAN_SCHEMA_VERSION,
                    sequence: opening_sequence + records.len() as u64 + 1,
                    tag_id: sample.tag_id,
                    wall_time_ms,
                    plant_elapsed_sec,
                    value: sample.value,
                    quality: sample.quality,
                    source: sample.source,
                };
                serde_json::to_writer(&mut encoded, &record)?;
                encoded.push(b'\n');
                records.push(record);
            }
        }
        inner.file.write_all(&encoded)?;
        inner.file.sync_data()?;
        inner.last_sequence = opening_sequence + records.len() as u64;
        let appended = records.len();
        for record in records {
            inner.recent.push_back(record);
            while inner.recent.len() > self.recent_capacity {
                inner.recent.pop_front();
            }
        }
        Ok(appended)
    }

    pub fn query(
        &self,
        tag_id: Option<&str>,
        since_ms: Option<u64>,
        limit: usize,
    ) -> Vec<HistorianSample> {
        let inner = self.inner.lock().expect("historian mutex poisoned");
        let mut matches: Vec<_> = inner
            .recent
            .iter()
            .rev()
            .filter(|sample| tag_id.is_none_or(|tag| sample.tag_id == tag))
            .filter(|sample| since_ms.is_none_or(|since| sample.wall_time_ms >= since))
            .take(limit.clamp(1, 10_000))
            .cloned()
            .collect();
        matches.reverse();
        matches
    }

    pub fn stats(&self) -> HistorianStats {
        let inner = self.inner.lock().expect("historian mutex poisoned");
        HistorianStats {
            schema_version: HISTORIAN_SCHEMA_VERSION,
            archive_path: self.archive_path.display().to_string(),
            durable_records: inner.last_sequence,
            recent_records: inner.recent.len(),
            recent_capacity: self.recent_capacity,
            first_recent_sequence: inner.recent.front().map(|record| record.sequence),
            last_sequence: inner.last_sequence,
        }
    }
}

fn recover_archive(
    path: &Path,
    recent_capacity: usize,
) -> Result<(VecDeque<HistorianSample>, u64)> {
    if !path.exists() {
        return Ok((VecDeque::with_capacity(recent_capacity.min(4096)), 0));
    }
    let payload = fs::read(path)
        .with_context(|| format!("failed to read historian archive {}", path.display()))?;
    let mut recent = VecDeque::with_capacity(recent_capacity.min(4096));
    let mut last_sequence = 0;
    let mut valid_end = 0;
    for (index, line) in payload.split_inclusive(|byte| *byte == b'\n').enumerate() {
        let complete = line.last() == Some(&b'\n');
        let content = if complete {
            &line[..line.len() - 1]
        } else {
            line
        };
        if content.iter().all(u8::is_ascii_whitespace) {
            valid_end += line.len();
            continue;
        }
        match serde_json::from_slice::<HistorianSample>(content) {
            Ok(record)
                if record.schema_version == HISTORIAN_SCHEMA_VERSION
                    && record.sequence > last_sequence =>
            {
                last_sequence = record.sequence;
                recent.push_back(record);
                while recent.len() > recent_capacity {
                    recent.pop_front();
                }
                valid_end += line.len();
            }
            Err(error) if !complete && valid_end + line.len() == payload.len() => {
                tracing::warn!(
                    "Discarding incomplete historian tail at record {}: {error}",
                    index + 1
                );
                break;
            }
            Ok(_) => bail!(
                "historian sequence/schema violation at record {}",
                index + 1
            ),
            Err(error) => bail!("invalid historian record {}: {error}", index + 1),
        }
    }
    if valid_end < payload.len() {
        OpenOptions::new()
            .write(true)
            .open(path)?
            .set_len(valid_end as u64)?;
    } else if !payload.is_empty() && payload.last() != Some(&b'\n') {
        let mut file = OpenOptions::new().append(true).open(path)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
    }
    Ok((recent, last_sequence))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "underhill-historian-{name}-{}-{}",
            std::process::id(),
            crate::persistence::wall_time_ms()
        ))
    }

    fn sample(tag: &str, value: f64) -> NewHistorianSample {
        NewHistorianSample {
            tag_id: tag.to_string(),
            value: Value::from(value),
            quality: TelemetryQuality::Good,
            source: "test_model".to_string(),
        }
    }

    #[test]
    fn archive_survives_reopen_and_continues_sequence() {
        let directory = state_dir("reopen");
        let historian = Historian::open_with_capacity(&directory, 10).unwrap();
        historian
            .append_batch(100, 1.0, vec![sample("tag.a", 1.0), sample("tag.b", 2.0)])
            .unwrap();
        drop(historian);
        let restored = Historian::open_with_capacity(&directory, 10).unwrap();
        assert_eq!(restored.stats().last_sequence, 2);
        restored
            .append_batch(200, 2.0, vec![sample("tag.a", 3.0)])
            .unwrap();
        assert_eq!(restored.stats().last_sequence, 3);
        let results = restored.query(Some("tag.a"), None, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].sequence, 1);
        assert_eq!(results[1].sequence, 3);
    }

    #[test]
    fn recent_window_is_bounded_without_losing_durable_count() {
        let directory = state_dir("bounded");
        let historian = Historian::open_with_capacity(&directory, 2).unwrap();
        historian
            .append_batch(
                100,
                1.0,
                vec![sample("tag", 1.0), sample("tag", 2.0), sample("tag", 3.0)],
            )
            .unwrap();
        let stats = historian.stats();
        assert_eq!(stats.durable_records, 3);
        assert_eq!(stats.recent_records, 2);
        assert_eq!(stats.first_recent_sequence, Some(2));
    }

    #[test]
    fn accelerated_frames_preserve_each_simulated_cadence_boundary() {
        let directory = state_dir("accelerated-frames");
        let historian = Historian::open_with_capacity(&directory, 20).unwrap();
        historian
            .append_frames(
                100,
                vec![
                    (1.0, vec![sample("tag", 1.0)]),
                    (2.0, vec![sample("tag", 2.0)]),
                    (3.0, vec![sample("tag", 3.0)]),
                ],
            )
            .unwrap();
        let records = historian.query(Some("tag"), None, 10);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].plant_elapsed_sec, 1.0);
        assert_eq!(records[1].plant_elapsed_sec, 2.0);
        assert_eq!(records[2].plant_elapsed_sec, 3.0);
    }
}
