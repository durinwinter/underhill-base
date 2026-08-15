use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

use crate::subsystems::WaterSnapshot;

pub const CAMPAIGN_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CampaignTemplateId {
    AirlockEqualizeStiction,
    SafetyCompoundLeakFire,
    MaintenanceSharedToolContention,
    WaterConductivityReplay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    Scheduled,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignObservation {
    pub agent_id: String,
    pub plant_elapsed_sec: f64,
    pub diagnosis: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignReport {
    pub expected_diagnosis: String,
    pub first_correct_observation_sec: Option<f64>,
    pub detection_latency_sec: Option<f64>,
    pub best_correct_confidence: Option<f64>,
    pub observation_count: usize,
    pub trace_qualification: Option<TraceQualificationReport>,
    pub water_replay_qualification: Option<WaterReplayQualificationReport>,
    pub passed: bool,
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterReplayTraceSample {
    pub plant_elapsed_sec: f64,
    pub true_conductivity_us_cm: f64,
    pub observed_conductivity_us_cm: f64,
    pub true_alarm_water_quality: bool,
    pub observed_alarm_water_quality: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterReplayQualificationReport {
    pub contract_id: String,
    pub evidence_class: String,
    pub dataset_id: String,
    pub sample_count: usize,
    pub true_excursion_us_cm: f64,
    pub observed_excursion_us_cm: f64,
    pub peak_truth_observation_divergence_us_cm: f64,
    pub concealed_alarm_samples: usize,
    pub passed: bool,
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirlockValveTraceSample {
    pub plant_elapsed_sec: f64,
    pub command_pct: f64,
    pub true_position_pct: f64,
    pub sensed_position_pct: f64,
    pub residual_pct: f64,
    pub stiction_active: bool,
    pub airlock_pressure_pa: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceQualificationReport {
    pub contract_id: String,
    pub evidence_class: String,
    pub dataset_id: String,
    pub sample_count: usize,
    pub command_excursion_pct: f64,
    pub true_position_excursion_pct: f64,
    pub peak_absolute_residual_pct: f64,
    pub stiction_active_fraction: f64,
    pub passed: bool,
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TraceQualificationContract {
    schema_version: u32,
    contract_id: String,
    dataset_id: String,
    evidence_class: String,
    source_artifact_sha256: Option<String>,
    sample_period_sec: f64,
    thresholds: TraceQualificationThresholds,
    claim_boundary: String,
}

#[derive(Debug, Deserialize)]
struct TraceQualificationThresholds {
    minimum_samples: usize,
    minimum_command_excursion_pct: f64,
    minimum_true_position_excursion_pct: f64,
    minimum_peak_absolute_residual_pct: f64,
    minimum_stiction_active_fraction: f64,
}

#[derive(Debug, Deserialize)]
struct WaterReplayQualificationContract {
    schema_version: u32,
    contract_id: String,
    dataset_id: String,
    evidence_class: String,
    source_artifact_sha256: Option<String>,
    sample_period_sec: f64,
    thresholds: WaterReplayQualificationThresholds,
    claim_boundary: String,
}

#[derive(Debug, Deserialize)]
struct WaterReplayQualificationThresholds {
    minimum_samples: usize,
    minimum_true_excursion_us_cm: f64,
    maximum_observed_excursion_us_cm: f64,
    minimum_peak_truth_observation_divergence_us_cm: f64,
    minimum_concealed_alarm_samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCampaign {
    pub schema_version: u32,
    pub campaign_id: String,
    pub template_id: CampaignTemplateId,
    pub dataset_id: String,
    pub target_pea: String,
    pub seed: u64,
    pub created_plant_sec: f64,
    pub fault_onset_plant_sec: f64,
    pub planned_end_plant_sec: f64,
    pub actual_end_plant_sec: Option<f64>,
    pub status: CampaignStatus,
    pub ground_truth: Value,
    pub pre_campaign_baseline: Option<Value>,
    pub observations: Vec<CampaignObservation>,
    #[serde(default)]
    pub trace_samples: Vec<AirlockValveTraceSample>,
    #[serde(default)]
    pub water_replay_trace_samples: Vec<WaterReplayTraceSample>,
    #[serde(default)]
    stimulus_stage: u8,
    pub report: Option<CampaignReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCampaignRequest {
    pub template_id: CampaignTemplateId,
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "default_onset_delay_sec")]
    pub fault_onset_delay_sec: f64,
    #[serde(default = "default_duration_sec")]
    pub duration_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitObservationRequest {
    pub agent_id: String,
    pub diagnosis: String,
    pub confidence: f64,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CampaignAction {
    Activate {
        campaign_id: String,
        template_id: CampaignTemplateId,
        ground_truth: Value,
    },
    StimulateAirlockValve {
        campaign_id: String,
        command_pct: f64,
    },
    Complete {
        campaign_id: String,
        template_id: CampaignTemplateId,
        baseline: Value,
        report: CampaignReport,
    },
}

impl CampaignAction {
    pub fn campaign_id(&self) -> &str {
        match self {
            Self::Activate { campaign_id, .. }
            | Self::StimulateAirlockValve { campaign_id, .. }
            | Self::Complete { campaign_id, .. } => campaign_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignManager {
    pub schema_version: u32,
    next_campaign_sequence: u64,
    campaigns: Vec<ValidationCampaign>,
}

impl Default for CampaignManager {
    fn default() -> Self {
        Self {
            schema_version: CAMPAIGN_SCHEMA_VERSION,
            next_campaign_sequence: 1,
            campaigns: Vec::new(),
        }
    }
}

impl CampaignManager {
    pub fn prepare_after_restore(&mut self) {
        self.schema_version = CAMPAIGN_SCHEMA_VERSION;
        self.next_campaign_sequence = self.next_campaign_sequence.max(
            self.campaigns
                .iter()
                .filter_map(|campaign| campaign.campaign_id.strip_prefix("campaign-"))
                .filter_map(|suffix| suffix.parse::<u64>().ok())
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
    }

    pub fn campaigns(&self) -> &[ValidationCampaign] {
        &self.campaigns
    }

    pub fn get(&self, campaign_id: &str) -> Option<&ValidationCampaign> {
        self.campaigns
            .iter()
            .find(|campaign| campaign.campaign_id == campaign_id)
    }

    pub fn public_view(campaign: &ValidationCampaign) -> Value {
        let mut value = serde_json::to_value(campaign).unwrap_or_else(|_| serde_json::json!({}));
        let Some(object) = value.as_object_mut() else {
            return value;
        };
        object.remove("pre_campaign_baseline");
        object.remove("stimulus_stage");
        object.remove("trace_samples");
        object.remove("water_replay_trace_samples");
        object.insert(
            "trace_sample_count".to_string(),
            serde_json::json!(
                campaign.trace_samples.len() + campaign.water_replay_trace_samples.len()
            ),
        );
        if matches!(
            campaign.status,
            CampaignStatus::Scheduled | CampaignStatus::Active
        ) {
            object.remove("template_id");
            object.remove("dataset_id");
            object.remove("seed");
            object.remove("ground_truth");
            object.remove("observations");
            object.remove("report");
            object.insert(
                "observation_count".to_string(),
                serde_json::json!(campaign.observations.len()),
            );
            object.insert("evaluation_blinded".to_string(), Value::Bool(true));
        } else {
            object.insert("evaluation_blinded".to_string(), Value::Bool(false));
        }
        value
    }

    pub fn create(
        &mut self,
        request: CreateCampaignRequest,
        plant_elapsed_sec: f64,
    ) -> Result<ValidationCampaign, String> {
        if !plant_elapsed_sec.is_finite() || plant_elapsed_sec < 0.0 {
            return Err("plant time must be finite and non-negative".to_string());
        }
        if !request.fault_onset_delay_sec.is_finite()
            || !(1.0..=86_400.0).contains(&request.fault_onset_delay_sec)
        {
            return Err("fault_onset_delay_sec must be within 1..=86400".to_string());
        }
        if !request.duration_sec.is_finite() || !(5.0..=604_800.0).contains(&request.duration_sec) {
            return Err("duration_sec must be within 5..=604800".to_string());
        }
        if self.campaigns.iter().any(|campaign| {
            matches!(
                campaign.status,
                CampaignStatus::Scheduled | CampaignStatus::Active
            )
        }) {
            return Err(
                "only one fault campaign may own live injection surfaces at a time".to_string(),
            );
        }
        if request.template_id == CampaignTemplateId::AirlockEqualizeStiction
            && !(35.0..=600.0).contains(&request.duration_sec)
        {
            return Err(
                "airlock stiction trace campaigns require duration_sec within 35..=600".to_string(),
            );
        }
        if request.template_id == CampaignTemplateId::WaterConductivityReplay
            && !(60.0..=600.0).contains(&request.duration_sec)
        {
            return Err(
                "water conductivity replay campaigns require duration_sec within 60..=600"
                    .to_string(),
            );
        }
        let (dataset_id, target_pea, ground_truth) =
            template_metadata(request.template_id, request.seed);
        let onset = plant_elapsed_sec + request.fault_onset_delay_sec;
        let campaign = ValidationCampaign {
            schema_version: CAMPAIGN_SCHEMA_VERSION,
            campaign_id: format!("campaign-{:06}", self.next_campaign_sequence),
            template_id: request.template_id,
            dataset_id: dataset_id.to_string(),
            target_pea: target_pea.to_string(),
            seed: request.seed,
            created_plant_sec: plant_elapsed_sec,
            fault_onset_plant_sec: onset,
            planned_end_plant_sec: onset + request.duration_sec,
            actual_end_plant_sec: None,
            status: CampaignStatus::Scheduled,
            ground_truth,
            pre_campaign_baseline: None,
            observations: Vec::new(),
            trace_samples: Vec::new(),
            water_replay_trace_samples: Vec::new(),
            stimulus_stage: 0,
            report: None,
        };
        self.next_campaign_sequence += 1;
        self.campaigns.push(campaign.clone());
        Ok(campaign)
    }

    pub fn submit_observation(
        &mut self,
        campaign_id: &str,
        request: SubmitObservationRequest,
        plant_elapsed_sec: f64,
    ) -> Result<ValidationCampaign, String> {
        if request.agent_id.trim().is_empty() || request.diagnosis.trim().is_empty() {
            return Err("agent_id and diagnosis are required".to_string());
        }
        if !request.confidence.is_finite() || !(0.0..=1.0).contains(&request.confidence) {
            return Err("confidence must be within 0..=1".to_string());
        }
        let campaign = self
            .campaigns
            .iter_mut()
            .find(|campaign| campaign.campaign_id == campaign_id)
            .ok_or_else(|| format!("campaign not found: {campaign_id}"))?;
        if matches!(
            campaign.status,
            CampaignStatus::Completed | CampaignStatus::Cancelled
        ) {
            return Err("campaign no longer accepts observations".to_string());
        }
        campaign.observations.push(CampaignObservation {
            agent_id: request.agent_id,
            plant_elapsed_sec,
            diagnosis: request.diagnosis,
            confidence: request.confidence,
            evidence: request.evidence,
            recommendation: request.recommendation,
        });
        Ok(campaign.clone())
    }

    pub fn cancel(
        &mut self,
        campaign_id: &str,
        plant_elapsed_sec: f64,
        reason: String,
    ) -> Result<(), String> {
        let campaign = self
            .campaigns
            .iter_mut()
            .find(|campaign| campaign.campaign_id == campaign_id)
            .ok_or_else(|| format!("campaign not found: {campaign_id}"))?;
        if matches!(
            campaign.status,
            CampaignStatus::Completed | CampaignStatus::Cancelled
        ) {
            return Ok(());
        }
        campaign.status = CampaignStatus::Cancelled;
        campaign.actual_end_plant_sec = Some(plant_elapsed_sec);
        campaign.report = Some(CampaignReport {
            expected_diagnosis: campaign
                .ground_truth
                .get("expected_diagnosis")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            first_correct_observation_sec: None,
            detection_latency_sec: None,
            best_correct_confidence: None,
            observation_count: campaign.observations.len(),
            trace_qualification: None,
            water_replay_qualification: None,
            passed: false,
            failure_reasons: vec![reason],
        });
        Ok(())
    }

    pub fn set_baseline(&mut self, campaign_id: &str, baseline: Value) -> Result<(), String> {
        let campaign = self
            .campaigns
            .iter_mut()
            .find(|campaign| campaign.campaign_id == campaign_id)
            .ok_or_else(|| format!("campaign not found: {campaign_id}"))?;
        if campaign.status != CampaignStatus::Active {
            return Err("baseline can only be recorded for an active campaign".to_string());
        }
        campaign.pre_campaign_baseline = Some(baseline);
        Ok(())
    }

    pub fn record_airlock_trace(
        &mut self,
        plant_elapsed_sec: f64,
        command_pct: f64,
        true_position_pct: f64,
        sensed_position_pct: f64,
        residual_pct: f64,
        stiction_active: bool,
        airlock_pressure_pa: f64,
    ) {
        if ![
            plant_elapsed_sec,
            command_pct,
            true_position_pct,
            sensed_position_pct,
            residual_pct,
            airlock_pressure_pa,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return;
        }
        for campaign in &mut self.campaigns {
            if campaign.status == CampaignStatus::Active
                && campaign.template_id == CampaignTemplateId::AirlockEqualizeStiction
                && plant_elapsed_sec >= campaign.fault_onset_plant_sec
            {
                campaign.trace_samples.push(AirlockValveTraceSample {
                    plant_elapsed_sec,
                    command_pct,
                    true_position_pct,
                    sensed_position_pct,
                    residual_pct,
                    stiction_active,
                    airlock_pressure_pa,
                });
            }
        }
    }

    pub fn observed_water_snapshot(&self, mut snapshot: WaterSnapshot) -> WaterSnapshot {
        if let Some(frozen_value) = self.campaigns.iter().find_map(|campaign| {
            (campaign.status == CampaignStatus::Active
                && campaign.template_id == CampaignTemplateId::WaterConductivityReplay)
                .then(|| {
                    campaign
                        .pre_campaign_baseline
                        .as_ref()
                        .and_then(|baseline| baseline.get("frozen_conductivity_us_cm"))
                        .and_then(Value::as_f64)
                })
                .flatten()
        }) {
            snapshot.potable_conductivity_us_cm = frozen_value;
            snapshot.alarm_water_quality = snapshot.potable_conductivity_us_cm > 500.0
                || snapshot.potable_toc_mg_l > 2.0
                || snapshot.microbial_cfu_ml > 100.0;
        }
        snapshot
    }

    pub fn record_water_replay_trace(
        &mut self,
        plant_elapsed_sec: f64,
        true_snapshot: &WaterSnapshot,
        observed_snapshot: &WaterSnapshot,
    ) {
        if !plant_elapsed_sec.is_finite()
            || !true_snapshot.potable_conductivity_us_cm.is_finite()
            || !observed_snapshot.potable_conductivity_us_cm.is_finite()
        {
            return;
        }
        for campaign in &mut self.campaigns {
            if campaign.status == CampaignStatus::Active
                && campaign.template_id == CampaignTemplateId::WaterConductivityReplay
                && plant_elapsed_sec >= campaign.fault_onset_plant_sec
            {
                campaign
                    .water_replay_trace_samples
                    .push(WaterReplayTraceSample {
                        plant_elapsed_sec,
                        true_conductivity_us_cm: true_snapshot.potable_conductivity_us_cm,
                        observed_conductivity_us_cm: observed_snapshot.potable_conductivity_us_cm,
                        true_alarm_water_quality: true_snapshot.alarm_water_quality,
                        observed_alarm_water_quality: observed_snapshot.alarm_water_quality,
                    });
            }
        }
    }

    pub fn advance(&mut self, plant_elapsed_sec: f64) -> Vec<CampaignAction> {
        let mut actions = Vec::new();
        for campaign in &mut self.campaigns {
            if campaign.status == CampaignStatus::Scheduled
                && plant_elapsed_sec >= campaign.fault_onset_plant_sec
            {
                campaign.status = CampaignStatus::Active;
                campaign.stimulus_stage = 1;
                actions.push(CampaignAction::Activate {
                    campaign_id: campaign.campaign_id.clone(),
                    template_id: campaign.template_id,
                    ground_truth: campaign.ground_truth.clone(),
                });
            }
            if campaign.status == CampaignStatus::Active
                && campaign.template_id == CampaignTemplateId::AirlockEqualizeStiction
                && let Some(commands) = campaign
                    .pre_campaign_baseline
                    .as_ref()
                    .and_then(|baseline| baseline.get("stimulus_commands_pct"))
                    .and_then(Value::as_array)
            {
                let elapsed = plant_elapsed_sec - campaign.fault_onset_plant_sec;
                let next_stage = if elapsed >= 25.0 {
                    3
                } else if elapsed >= 10.0 {
                    2
                } else {
                    campaign.stimulus_stage
                };
                if next_stage > campaign.stimulus_stage
                    && let Some(command_pct) = commands
                        .get((next_stage - 1) as usize)
                        .and_then(Value::as_f64)
                {
                    campaign.stimulus_stage = next_stage;
                    actions.push(CampaignAction::StimulateAirlockValve {
                        campaign_id: campaign.campaign_id.clone(),
                        command_pct,
                    });
                }
            }
            if campaign.status == CampaignStatus::Active
                && plant_elapsed_sec >= campaign.planned_end_plant_sec
                && let Some(baseline) = campaign.pre_campaign_baseline.clone()
            {
                campaign.status = CampaignStatus::Completed;
                campaign.actual_end_plant_sec = Some(plant_elapsed_sec);
                let report = score_campaign(campaign);
                campaign.report = Some(report.clone());
                actions.push(CampaignAction::Complete {
                    campaign_id: campaign.campaign_id.clone(),
                    template_id: campaign.template_id,
                    baseline,
                    report,
                });
            }
        }
        actions
    }
}

fn score_campaign(campaign: &ValidationCampaign) -> CampaignReport {
    let expected = campaign
        .ground_truth
        .get("expected_diagnosis")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let matching = campaign.observations.iter().filter(|observation| {
        observation.plant_elapsed_sec >= campaign.fault_onset_plant_sec
            && normalize(&observation.diagnosis) == normalize(&expected)
    });
    let first = matching
        .clone()
        .map(|observation| observation.plant_elapsed_sec)
        .min_by(f64::total_cmp);
    let best_confidence = matching
        .map(|observation| observation.confidence)
        .max_by(f64::total_cmp);
    let latency = first.map(|time| (time - campaign.fault_onset_plant_sec).max(0.0));
    let latency_limit = match campaign.template_id {
        CampaignTemplateId::AirlockEqualizeStiction => 30.0,
        CampaignTemplateId::SafetyCompoundLeakFire => 15.0,
        CampaignTemplateId::MaintenanceSharedToolContention => 20.0,
        CampaignTemplateId::WaterConductivityReplay => 15.0,
    };
    let mut failure_reasons = Vec::new();
    if latency.is_none() {
        failure_reasons.push("expected diagnosis was not observed after fault onset".to_string());
    } else if latency.is_some_and(|value| value > latency_limit) {
        failure_reasons.push(format!(
            "detection latency exceeded {latency_limit:.0} simulated seconds"
        ));
    }
    if best_confidence.is_some_and(|confidence| confidence < 0.5) {
        failure_reasons.push("best correct-diagnosis confidence was below 0.5".to_string());
    }
    let trace_qualification = if campaign.template_id == CampaignTemplateId::AirlockEqualizeStiction
    {
        let report = qualify_airlock_stiction_trace(&campaign.trace_samples);
        if !report.passed {
            failure_reasons.push(
                "physical trace did not satisfy the native stiction qualification contract"
                    .to_string(),
            );
        }
        Some(report)
    } else {
        None
    };
    let water_replay_qualification =
        if campaign.template_id == CampaignTemplateId::WaterConductivityReplay {
            let report = qualify_water_replay_trace(&campaign.water_replay_trace_samples);
            if !report.passed {
                failure_reasons.push(
                "physical/observed trace did not satisfy the native replay qualification contract"
                    .to_string(),
            );
            }
            Some(report)
        } else {
            None
        };
    CampaignReport {
        expected_diagnosis: expected,
        first_correct_observation_sec: first,
        detection_latency_sec: latency,
        best_correct_confidence: best_confidence,
        observation_count: campaign.observations.len(),
        trace_qualification,
        water_replay_qualification,
        passed: failure_reasons.is_empty(),
        failure_reasons,
    }
}

fn qualify_water_replay_trace(
    samples: &[WaterReplayTraceSample],
) -> WaterReplayQualificationReport {
    let contract = water_replay_qualification_contract();
    let thresholds = &contract.thresholds;
    let range = |values: Vec<f64>| {
        let minimum = values.iter().copied().min_by(f64::total_cmp).unwrap_or(0.0);
        let maximum = values.iter().copied().max_by(f64::total_cmp).unwrap_or(0.0);
        maximum - minimum
    };
    let true_excursion_us_cm = range(
        samples
            .iter()
            .map(|sample| sample.true_conductivity_us_cm)
            .collect(),
    );
    let observed_excursion_us_cm = range(
        samples
            .iter()
            .map(|sample| sample.observed_conductivity_us_cm)
            .collect(),
    );
    let peak_truth_observation_divergence_us_cm = samples
        .iter()
        .map(|sample| (sample.true_conductivity_us_cm - sample.observed_conductivity_us_cm).abs())
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let concealed_alarm_samples = samples
        .iter()
        .filter(|sample| sample.true_alarm_water_quality && !sample.observed_alarm_water_quality)
        .count();
    let mut failure_reasons = Vec::new();
    if samples.len() < thresholds.minimum_samples {
        failure_reasons.push(format!(
            "fewer than {} one-second replay samples",
            thresholds.minimum_samples
        ));
    }
    if true_excursion_us_cm < thresholds.minimum_true_excursion_us_cm {
        failure_reasons.push(format!(
            "true conductivity excursion below {:.0} uS/cm",
            thresholds.minimum_true_excursion_us_cm
        ));
    }
    if observed_excursion_us_cm > thresholds.maximum_observed_excursion_us_cm {
        failure_reasons.push(format!(
            "replayed conductivity changed by more than {:.0} uS/cm",
            thresholds.maximum_observed_excursion_us_cm
        ));
    }
    if peak_truth_observation_divergence_us_cm
        < thresholds.minimum_peak_truth_observation_divergence_us_cm
    {
        failure_reasons.push(format!(
            "truth/observation divergence below {:.0} uS/cm",
            thresholds.minimum_peak_truth_observation_divergence_us_cm
        ));
    }
    if concealed_alarm_samples < thresholds.minimum_concealed_alarm_samples {
        failure_reasons.push("no physically active water-quality alarm was concealed".to_string());
    }
    WaterReplayQualificationReport {
        contract_id: contract.contract_id.clone(),
        evidence_class: contract.evidence_class.clone(),
        dataset_id: contract.dataset_id.clone(),
        sample_count: samples.len(),
        true_excursion_us_cm,
        observed_excursion_us_cm,
        peak_truth_observation_divergence_us_cm,
        concealed_alarm_samples,
        passed: failure_reasons.is_empty(),
        failure_reasons,
    }
}

fn water_replay_qualification_contract() -> &'static WaterReplayQualificationContract {
    static CONTRACT: OnceLock<WaterReplayQualificationContract> = OnceLock::new();
    CONTRACT.get_or_init(|| {
        let contract: WaterReplayQualificationContract = serde_json::from_str(include_str!(
            "../../../validation/datasets/contracts/underhill-water-conductivity-replay-v1.json"
        ))
        .expect("embedded water replay qualification contract must parse");
        assert_eq!(contract.schema_version, 1);
        assert_eq!(contract.sample_period_sec, 1.0);
        assert!(contract.source_artifact_sha256.is_none());
        assert!(!contract.claim_boundary.trim().is_empty());
        contract
    })
}

fn qualify_airlock_stiction_trace(samples: &[AirlockValveTraceSample]) -> TraceQualificationReport {
    let contract = airlock_stiction_trace_contract();
    let thresholds = &contract.thresholds;

    let range = |values: Vec<f64>| {
        let minimum = values.iter().copied().min_by(f64::total_cmp).unwrap_or(0.0);
        let maximum = values.iter().copied().max_by(f64::total_cmp).unwrap_or(0.0);
        maximum - minimum
    };
    let command_excursion_pct = range(samples.iter().map(|sample| sample.command_pct).collect());
    let true_position_excursion_pct = range(
        samples
            .iter()
            .map(|sample| sample.true_position_pct)
            .collect(),
    );
    let peak_absolute_residual_pct = samples
        .iter()
        .map(|sample| sample.residual_pct.abs())
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let stiction_active_fraction = if samples.is_empty() {
        0.0
    } else {
        samples
            .iter()
            .filter(|sample| sample.stiction_active)
            .count() as f64
            / samples.len() as f64
    };
    let mut failure_reasons = Vec::new();
    if samples.len() < thresholds.minimum_samples {
        failure_reasons.push(format!(
            "fewer than {} one-second trace samples",
            thresholds.minimum_samples
        ));
    }
    if command_excursion_pct < thresholds.minimum_command_excursion_pct {
        failure_reasons.push(format!(
            "command excursion below {:.0}%",
            thresholds.minimum_command_excursion_pct
        ));
    }
    if true_position_excursion_pct < thresholds.minimum_true_position_excursion_pct {
        failure_reasons.push(format!(
            "true-position excursion below {:.0}%",
            thresholds.minimum_true_position_excursion_pct
        ));
    }
    if peak_absolute_residual_pct < thresholds.minimum_peak_absolute_residual_pct {
        failure_reasons.push(format!(
            "peak residual below {:.0}%",
            thresholds.minimum_peak_absolute_residual_pct
        ));
    }
    if stiction_active_fraction < thresholds.minimum_stiction_active_fraction {
        failure_reasons.push(format!(
            "stiction-active fraction below {:.2}",
            thresholds.minimum_stiction_active_fraction
        ));
    }
    TraceQualificationReport {
        contract_id: contract.contract_id.clone(),
        evidence_class: contract.evidence_class.clone(),
        dataset_id: contract.dataset_id.clone(),
        sample_count: samples.len(),
        command_excursion_pct,
        true_position_excursion_pct,
        peak_absolute_residual_pct,
        stiction_active_fraction,
        passed: failure_reasons.is_empty(),
        failure_reasons,
    }
}

fn airlock_stiction_trace_contract() -> &'static TraceQualificationContract {
    static CONTRACT: OnceLock<TraceQualificationContract> = OnceLock::new();
    CONTRACT.get_or_init(|| {
        let contract: TraceQualificationContract = serde_json::from_str(include_str!(
            "../../../validation/datasets/contracts/underhill-airlock-stiction-trace-v1.json"
        ))
        .expect("embedded stiction trace contract must parse");
        assert_eq!(contract.schema_version, 1);
        assert_eq!(contract.sample_period_sec, 1.0);
        assert!(contract.source_artifact_sha256.is_none());
        assert!(!contract.claim_boundary.trim().is_empty());
        contract
    })
}

fn template_metadata(
    template_id: CampaignTemplateId,
    seed: u64,
) -> (&'static str, &'static str, Value) {
    match template_id {
        CampaignTemplateId::AirlockEqualizeStiction => (
            "damadics-actuator-benchmark",
            "AIRLOCK-PEA-001",
            serde_json::json!({
                "expected_diagnosis": "equalize_valve_stiction",
                "valve": "equalize",
                "stiction_breakaway_pct": 14.0 + (seed % 9) as f64,
                "trace_contract_id": "underhill-airlock-stiction-trace-v1",
                "evidence_class": "underhill_native_damadics_informed"
            }),
        ),
        CampaignTemplateId::SafetyCompoundLeakFire => (
            "swat-wadi-batadal-security-family",
            "SAFETY-PEA-001",
            serde_json::json!({
                "expected_diagnosis": "compound_habitat_leak_and_fire",
                "injected_leak_kg_s": 0.01,
                "fire_source_kw": 30.0,
                "habitat_isolated": false
            }),
        ),
        CampaignTemplateId::MaintenanceSharedToolContention => (
            "ices-2025-127-eclss-reliability",
            "ROBOTICS-PEA-001",
            serde_json::json!({
                "expected_diagnosis": "water_loop_tool_contention",
                "required_tool": "water_loop_service_kit",
                "injected_work_orders": 2,
                "available_tools": 1
            }),
        ),
        CampaignTemplateId::WaterConductivityReplay => (
            "swat-wadi-batadal-security-family",
            "WATER-PEA-001",
            serde_json::json!({
                "expected_diagnosis": "potable_conductivity_sensor_replay",
                "signal": "potable_conductivity_us_cm",
                "physical_contamination_target_us_cm": 800.0 + (seed % 5) as f64 * 25.0,
                "trace_contract_id": "underhill-water-conductivity-replay-v1",
                "evidence_class": "underhill_native_swat_wadi_batadal_informed"
            }),
        ),
    }
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn default_onset_delay_sec() -> f64 {
    5.0
}

fn default_duration_sec() -> f64 {
    120.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_qualified_stiction_trace(manager: &mut CampaignManager, onset_sec: f64) {
        for index in 0..35 {
            let (command, actual, residual, stuck) = if index < 10 {
                (10.0, 0.0, 10.0, true)
            } else if index < 25 {
                (50.0, 40.0, 10.0, false)
            } else {
                (38.0, 50.0, -12.0, true)
            };
            manager.record_airlock_trace(
                onset_sec + index as f64,
                command,
                actual,
                actual,
                residual,
                stuck,
                101_300.0,
            );
        }
    }

    #[test]
    fn campaign_uses_plant_time_and_requires_baseline_before_completion() {
        let mut manager = CampaignManager::default();
        let campaign = manager
            .create(
                CreateCampaignRequest {
                    template_id: CampaignTemplateId::SafetyCompoundLeakFire,
                    seed: 42,
                    fault_onset_delay_sec: 5.0,
                    duration_sec: 10.0,
                },
                100.0,
            )
            .unwrap();
        assert!(manager.advance(104.0).is_empty());
        assert!(matches!(
            manager.advance(105.0).as_slice(),
            [CampaignAction::Activate { .. }]
        ));
        assert!(manager.advance(115.0).is_empty());
        manager
            .set_baseline(&campaign.campaign_id, serde_json::json!({"fire": 0.0}))
            .unwrap();
        assert!(matches!(
            manager.advance(115.0).as_slice(),
            [CampaignAction::Complete { .. }]
        ));
    }

    #[test]
    fn correct_held_out_diagnosis_is_scored_in_simulated_time() {
        let mut manager = CampaignManager::default();
        let campaign = manager
            .create(
                CreateCampaignRequest {
                    template_id: CampaignTemplateId::AirlockEqualizeStiction,
                    seed: 7,
                    fault_onset_delay_sec: 5.0,
                    duration_sec: 40.0,
                },
                0.0,
            )
            .unwrap();
        manager.advance(5.0);
        manager
            .set_baseline(&campaign.campaign_id, serde_json::json!({}))
            .unwrap();
        manager
            .submit_observation(
                &campaign.campaign_id,
                SubmitObservationRequest {
                    agent_id: "agent-a".to_string(),
                    diagnosis: "Equalize valve stiction".to_string(),
                    confidence: 0.9,
                    evidence: vec!["position residual".to_string()],
                    recommendation: None,
                },
                12.0,
            )
            .unwrap();
        record_qualified_stiction_trace(&mut manager, 5.0);
        manager.advance(45.0);
        let report = manager
            .get(&campaign.campaign_id)
            .unwrap()
            .report
            .as_ref()
            .unwrap();
        assert!(report.passed);
        assert_eq!(report.detection_latency_sec, Some(7.0));
    }

    #[test]
    fn maintenance_contention_template_scores_against_its_live_latency_gate() {
        let mut manager = CampaignManager::default();
        let campaign = manager
            .create(
                CreateCampaignRequest {
                    template_id: CampaignTemplateId::MaintenanceSharedToolContention,
                    seed: 11,
                    fault_onset_delay_sec: 2.0,
                    duration_sec: 30.0,
                },
                100.0,
            )
            .unwrap();
        manager.advance(102.0);
        manager
            .set_baseline(
                &campaign.campaign_id,
                serde_json::json!({"work_order_ids": ["a", "b"]}),
            )
            .unwrap();
        manager
            .submit_observation(
                &campaign.campaign_id,
                SubmitObservationRequest {
                    agent_id: "maintenance-agent".to_string(),
                    diagnosis: "water loop tool contention".to_string(),
                    confidence: 0.9,
                    evidence: vec!["blocked work order".to_string()],
                    recommendation: Some("preserve priority".to_string()),
                },
                121.0,
            )
            .unwrap();
        manager.advance(132.0);
        let report = manager
            .get(&campaign.campaign_id)
            .unwrap()
            .report
            .as_ref()
            .unwrap();
        assert!(report.passed);
        assert_eq!(report.detection_latency_sec, Some(19.0));
    }

    #[test]
    fn concurrent_live_campaigns_are_rejected() {
        let mut manager = CampaignManager::default();
        let request = CreateCampaignRequest {
            template_id: CampaignTemplateId::AirlockEqualizeStiction,
            seed: 1,
            fault_onset_delay_sec: 5.0,
            duration_sec: 40.0,
        };
        manager.create(request.clone(), 0.0).unwrap();
        assert!(manager.create(request, 1.0).is_err());
    }

    #[test]
    fn active_campaign_and_baseline_survive_checkpoint_serialization() {
        let mut manager = CampaignManager::default();
        let campaign = manager
            .create(
                CreateCampaignRequest {
                    template_id: CampaignTemplateId::AirlockEqualizeStiction,
                    seed: 9,
                    fault_onset_delay_sec: 2.0,
                    duration_sec: 40.0,
                },
                50.0,
            )
            .unwrap();
        manager.advance(52.0);
        let baseline = serde_json::json!({"stiction_breakaway_pct": 0.0});
        manager
            .set_baseline(&campaign.campaign_id, baseline.clone())
            .unwrap();

        let encoded = serde_json::to_vec(&manager).unwrap();
        let mut restored: CampaignManager = serde_json::from_slice(&encoded).unwrap();
        let actions = restored.advance(92.0);

        assert!(matches!(
            actions.as_slice(),
            [CampaignAction::Complete {
                baseline: restored_baseline,
                ..
            }] if restored_baseline == &baseline
        ));
        assert_eq!(
            restored.get(&campaign.campaign_id).unwrap().status,
            CampaignStatus::Completed
        );
    }

    #[test]
    fn active_campaign_view_hides_ground_truth_and_other_agent_answers() {
        let mut manager = CampaignManager::default();
        let campaign = manager
            .create(
                CreateCampaignRequest {
                    template_id: CampaignTemplateId::AirlockEqualizeStiction,
                    seed: 77,
                    fault_onset_delay_sec: 2.0,
                    duration_sec: 40.0,
                },
                10.0,
            )
            .unwrap();
        manager.advance(12.0);
        manager
            .submit_observation(
                &campaign.campaign_id,
                SubmitObservationRequest {
                    agent_id: "agent-a".to_string(),
                    diagnosis: "equalize_valve_stiction".to_string(),
                    confidence: 0.8,
                    evidence: vec![],
                    recommendation: None,
                },
                13.0,
            )
            .unwrap();
        let view = CampaignManager::public_view(manager.get(&campaign.campaign_id).unwrap());
        assert_eq!(view["evaluation_blinded"], true);
        assert_eq!(view["observation_count"], 1);
        assert!(view.get("ground_truth").is_none());
        assert!(view.get("template_id").is_none());
        assert!(view.get("observations").is_none());
        assert!(view.get("pre_campaign_baseline").is_none());
    }

    #[test]
    fn water_replay_scores_hidden_truth_against_frozen_observation() {
        let mut manager = CampaignManager::default();
        let campaign = manager
            .create(
                CreateCampaignRequest {
                    template_id: CampaignTemplateId::WaterConductivityReplay,
                    seed: 12,
                    fault_onset_delay_sec: 2.0,
                    duration_sec: 60.0,
                },
                100.0,
            )
            .unwrap();
        manager.advance(102.0);
        manager
            .set_baseline(
                &campaign.campaign_id,
                serde_json::json!({
                    "prior_contamination_target_us_cm": 0.0,
                    "frozen_conductivity_us_cm": 145.0
                }),
            )
            .unwrap();
        manager
            .submit_observation(
                &campaign.campaign_id,
                SubmitObservationRequest {
                    agent_id: "water-security-agent".to_string(),
                    diagnosis: "potable conductivity sensor replay".to_string(),
                    confidence: 0.91,
                    evidence: vec!["cross-sensor inconsistency".to_string()],
                    recommendation: Some("isolate telemetry channel".to_string()),
                },
                112.0,
            )
            .unwrap();
        for index in 0..60 {
            let mut truth = crate::subsystems::WaterSimulation::new().snapshot();
            truth.potable_conductivity_us_cm = 145.0 + index as f64 * 10.0;
            truth.alarm_water_quality = truth.potable_conductivity_us_cm > 500.0;
            let observed = manager.observed_water_snapshot(truth.clone());
            manager.record_water_replay_trace(102.0 + index as f64, &truth, &observed);
        }
        manager.advance(162.0);
        let report = manager
            .get(&campaign.campaign_id)
            .unwrap()
            .report
            .as_ref()
            .unwrap();
        assert!(report.passed);
        let replay = report.water_replay_qualification.as_ref().unwrap();
        assert!(replay.passed);
        assert_eq!(replay.observed_excursion_us_cm, 0.0);
        assert!(replay.concealed_alarm_samples > 0);
    }
}
