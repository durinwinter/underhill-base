use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAMPAIGN_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CampaignTemplateId {
    AirlockEqualizeStiction,
    SafetyCompoundLeakFire,
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
    pub passed: bool,
    pub failure_reasons: Vec<String>,
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
    },
    Complete {
        campaign_id: String,
        template_id: CampaignTemplateId,
        baseline: Value,
        report: CampaignReport,
    },
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
    pub fn campaigns(&self) -> &[ValidationCampaign] {
        &self.campaigns
    }

    pub fn get(&self, campaign_id: &str) -> Option<&ValidationCampaign> {
        self.campaigns
            .iter()
            .find(|campaign| campaign.campaign_id == campaign_id)
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
        let (dataset_id, target_pea, ground_truth) = template_metadata(request.template_id);
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

    pub fn advance(&mut self, plant_elapsed_sec: f64) -> Vec<CampaignAction> {
        let mut actions = Vec::new();
        for campaign in &mut self.campaigns {
            if campaign.status == CampaignStatus::Scheduled
                && plant_elapsed_sec >= campaign.fault_onset_plant_sec
            {
                campaign.status = CampaignStatus::Active;
                actions.push(CampaignAction::Activate {
                    campaign_id: campaign.campaign_id.clone(),
                    template_id: campaign.template_id,
                });
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
    CampaignReport {
        expected_diagnosis: expected,
        first_correct_observation_sec: first,
        detection_latency_sec: latency,
        best_correct_confidence: best_confidence,
        observation_count: campaign.observations.len(),
        passed: failure_reasons.is_empty(),
        failure_reasons,
    }
}

fn template_metadata(template_id: CampaignTemplateId) -> (&'static str, &'static str, Value) {
    match template_id {
        CampaignTemplateId::AirlockEqualizeStiction => (
            "damadics-actuator-benchmark",
            "AIRLOCK-PEA-001",
            serde_json::json!({
                "expected_diagnosis": "equalize_valve_stiction",
                "valve": "equalize",
                "stiction_breakaway_pct": 18.0
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
    fn concurrent_live_campaigns_are_rejected() {
        let mut manager = CampaignManager::default();
        let request = CreateCampaignRequest {
            template_id: CampaignTemplateId::AirlockEqualizeStiction,
            seed: 1,
            fault_onset_delay_sec: 5.0,
            duration_sec: 30.0,
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
                    duration_sec: 10.0,
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
        let actions = restored.advance(62.0);

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
}
