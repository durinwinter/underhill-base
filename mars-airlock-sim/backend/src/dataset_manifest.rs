use serde::Deserialize;

pub const DATASET_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct DatasetManifest {
    pub schema_version: u32,
    pub dataset_id: String,
    pub canonical_name: String,
    pub provenance_status: ProvenanceStatus,
    pub source: DatasetSource,
    pub license: LicenseRecord,
    pub underhill: UnderhillMapping,
    pub partitions: Vec<DatasetPartition>,
    pub acceptance_metrics: Vec<AcceptanceMetric>,
    pub prohibited_claims: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStatus {
    SourceSupplied,
    SourceVerified,
    PendingVerification,
}

#[derive(Debug, Deserialize)]
pub struct DatasetSource {
    pub publisher: String,
    pub landing_page: Option<String>,
    pub retrieved_at: Option<String>,
    pub checksum_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LicenseRecord {
    pub status: String,
    pub identifier: Option<String>,
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct UnderhillMapping {
    pub roles: Vec<String>,
    pub target_peas: Vec<String>,
    pub signals: Vec<SignalMapping>,
    pub domain_gaps: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SignalMapping {
    pub source_signal: String,
    pub underhill_tag: String,
    pub transform: String,
}

#[derive(Debug, Deserialize)]
pub struct DatasetPartition {
    pub name: String,
    pub purpose: String,
    pub selector: String,
}

#[derive(Debug, Deserialize)]
pub struct AcceptanceMetric {
    pub name: String,
    pub direction: String,
    pub threshold: f64,
    pub unit: String,
}

impl DatasetManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != DATASET_MANIFEST_SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema version {}",
                self.schema_version
            ));
        }
        if self.dataset_id.trim().is_empty() || self.canonical_name.trim().is_empty() {
            return Err("dataset identity must not be empty".to_string());
        }
        if self.provenance_status == ProvenanceStatus::SourceVerified
            && (self.source.landing_page.as_deref().unwrap_or("").is_empty()
                || self.source.retrieved_at.as_deref().unwrap_or("").is_empty())
        {
            return Err(
                "verified provenance requires a landing page and retrieval date".to_string(),
            );
        }
        if let Some(checksum) = &self.source.checksum_sha256
            && (checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err("SHA-256 checksum must contain 64 hexadecimal characters".to_string());
        }
        if self.source.publisher.trim().is_empty()
            || self.license.status.trim().is_empty()
            || self.license.notes.trim().is_empty()
        {
            return Err("publisher and license record are required".to_string());
        }
        if self
            .license
            .identifier
            .as_ref()
            .is_some_and(|identifier| identifier.trim().is_empty())
        {
            return Err("license identifier must be null or non-empty".to_string());
        }
        if self.underhill.roles.is_empty()
            || self.underhill.target_peas.is_empty()
            || self.underhill.signals.is_empty()
            || self.underhill.domain_gaps.is_empty()
        {
            return Err(
                "Underhill roles, targets, mappings, and domain gaps are required".to_string(),
            );
        }
        if self.partitions.len() < 2 {
            return Err("at least calibration and held-out partitions are required".to_string());
        }
        if self.acceptance_metrics.is_empty() || self.prohibited_claims.is_empty() {
            return Err("metrics and prohibited claims are required".to_string());
        }
        for signal in &self.underhill.signals {
            if signal.source_signal.trim().is_empty()
                || signal.underhill_tag.trim().is_empty()
                || signal.transform.trim().is_empty()
            {
                return Err("signal mappings must be complete".to_string());
            }
        }
        for partition in &self.partitions {
            if partition.name.trim().is_empty()
                || partition.purpose.trim().is_empty()
                || partition.selector.trim().is_empty()
            {
                return Err("dataset partitions must be reproducible".to_string());
            }
        }
        for metric in &self.acceptance_metrics {
            if metric.name.trim().is_empty()
                || !matches!(metric.direction.as_str(), "min" | "max")
                || !metric.threshold.is_finite()
                || metric.unit.trim().is_empty()
            {
                return Err(
                    "acceptance metrics must have a direction, finite threshold, and unit"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashSet, fs, path::PathBuf};

    #[test]
    fn all_reference_dataset_manifests_are_semantically_valid() {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("validation/datasets/manifests");
        let mut paths: Vec<_> = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect();
        paths.sort();
        assert!(!paths.is_empty());
        let mut ids = HashSet::new();
        for path in paths {
            let manifest: DatasetManifest =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            manifest
                .validate()
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(ids.insert(manifest.dataset_id), "duplicate dataset id");
        }
    }
}
