use serde::{Deserialize, Serialize};

const PROFILE_JSON: &str = include_str!("../../../validation/reliability/eclss-components.v1.json");
const DEFAULT_RNG_SEED: u64 = 0x5eed_ec15_5eed_2025;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclssReliabilityProfileFile {
    pub schema_version: u32,
    pub profile_id: String,
    pub source: ReliabilitySource,
    pub model: ReliabilityModelMetadata,
    pub components: Vec<EclssComponentProfile>,
    pub prohibited_claims: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilitySource {
    pub document_id: String,
    pub report_number: String,
    pub title: String,
    pub publisher: String,
    pub landing_page: String,
    pub retrieved_at: String,
    pub sha256: String,
    pub evidence_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityModelMetadata {
    pub failure_distribution: String,
    pub annual_failure_rate_formula: String,
    pub repair_distribution: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclssComponentProfile {
    pub component_id: String,
    pub display_name: String,
    pub function: EclssFunction,
    pub source_table: String,
    pub source_row: String,
    pub mtbf_hours: f64,
    pub mttr_hours: f64,
    pub k_factor: f64,
    pub duty_cycle: f64,
    pub duty_cycle_basis: String,
    pub quantity_installed: u32,
    pub quantity_installed_basis: String,
    pub initial_spares: u32,
    pub initial_spares_basis: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EclssFunction {
    OxygenGeneration,
    CarbonDioxideRemoval,
    HumidityControl,
    WaterRecovery,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentCondition {
    Operational,
    Degraded,
    Failed,
    Repairing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EclssComponentRuntime {
    profile: EclssComponentProfile,
    condition: ComponentCondition,
    health_pct: f64,
    unit_operating_hours: f64,
    cumulative_operating_hours: f64,
    failure_exposure_remaining_hours: f64,
    repair_remaining_hours: f64,
    spares_remaining: u32,
    failure_count: u64,
    completed_repairs: u64,
    active_work_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclssReliabilityState {
    pub schema_version: u32,
    pub profile_id: String,
    pub source_document_id: String,
    pub source_report_number: String,
    rng_state: u64,
    next_work_order_sequence: u64,
    components: Vec<EclssComponentRuntime>,
    #[serde(default)]
    pending_events: Vec<ReliabilityEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityEvent {
    pub event_kind: String,
    pub component_id: String,
    pub work_order_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EclssReliabilitySnapshot {
    pub schema_version: u32,
    pub profile_id: String,
    pub source_document_id: String,
    pub source_report_number: String,
    pub source_parameter_evidence_class: String,
    pub configuration_assumptions_present: bool,
    pub failed_count: usize,
    pub degraded_count: usize,
    pub repairing_count: usize,
    pub maintenance_backlog: usize,
    pub spares_remaining: u32,
    pub components: Vec<EclssComponentSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EclssComponentSnapshot {
    pub component_id: String,
    pub display_name: String,
    pub function: EclssFunction,
    pub condition: ComponentCondition,
    pub health_pct: f64,
    pub capacity_fraction: f64,
    pub unit_operating_hours: f64,
    pub cumulative_operating_hours: f64,
    pub reference_mtbf_hours: f64,
    pub reference_mttr_hours: f64,
    pub operational_k_factor: f64,
    pub configured_failures_per_year: f64,
    pub duty_cycle: f64,
    pub duty_cycle_basis: String,
    pub quantity_installed: u32,
    pub quantity_installed_basis: String,
    pub initial_spares_basis: String,
    pub source_table: String,
    pub source_row: String,
    pub source_parameter_evidence_class: String,
    pub spares_remaining: u32,
    pub repair_remaining_hours: f64,
    pub failure_count: u64,
    pub completed_repairs: u64,
    pub active_work_order: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct EclssCapacity {
    pub oxygen_generation: f64,
    pub carbon_dioxide_removal: f64,
    pub humidity_control: f64,
    pub water_recovery: f64,
}

impl Default for EclssReliabilityState {
    fn default() -> Self {
        Self::new_with_seed(DEFAULT_RNG_SEED)
    }
}

impl EclssReliabilityState {
    pub fn new_with_seed(seed: u64) -> Self {
        let profile = load_profile().expect("embedded ECLSS reliability profile must be valid");
        let mut rng_state = if seed == 0 { DEFAULT_RNG_SEED } else { seed };
        let components = profile
            .components
            .into_iter()
            .map(|component_profile| {
                let failure_exposure_remaining_hours =
                    sample_failure_exposure(&mut rng_state, &component_profile);
                EclssComponentRuntime {
                    spares_remaining: component_profile.initial_spares,
                    profile: component_profile,
                    condition: ComponentCondition::Operational,
                    health_pct: 100.0,
                    unit_operating_hours: 0.0,
                    cumulative_operating_hours: 0.0,
                    failure_exposure_remaining_hours,
                    repair_remaining_hours: 0.0,
                    failure_count: 0,
                    completed_repairs: 0,
                    active_work_order: None,
                }
            })
            .collect();
        Self {
            schema_version: profile.schema_version,
            profile_id: profile.profile_id,
            source_document_id: profile.source.document_id,
            source_report_number: profile.source.report_number,
            rng_state,
            next_work_order_sequence: 1,
            components,
            pending_events: Vec::new(),
        }
    }

    pub fn step(&mut self, dt_sec: f64, process_running: bool) -> EclssCapacity {
        if dt_sec <= 0.0 || !dt_sec.is_finite() {
            return self.capacity();
        }
        let dt_hours = dt_sec / 3_600.0;
        for component in &mut self.components {
            match component.condition {
                ComponentCondition::Repairing => {
                    component.repair_remaining_hours =
                        (component.repair_remaining_hours - dt_hours).max(0.0);
                    if component.repair_remaining_hours <= f64::EPSILON {
                        component.condition = ComponentCondition::Operational;
                        component.health_pct = 100.0;
                        component.unit_operating_hours = 0.0;
                        component.completed_repairs += 1;
                        component.active_work_order = None;
                        component.failure_exposure_remaining_hours =
                            sample_failure_exposure(&mut self.rng_state, &component.profile);
                        self.pending_events.push(ReliabilityEvent {
                            event_kind: "repair_completed".to_string(),
                            component_id: component.profile.component_id.clone(),
                            work_order_id: None,
                            detail: "replacement ORU returned to operational service".to_string(),
                        });
                    }
                }
                ComponentCondition::Operational | ComponentCondition::Degraded
                    if process_running =>
                {
                    let exposure_hours = dt_hours * component.profile.duty_cycle;
                    component.unit_operating_hours += exposure_hours;
                    component.cumulative_operating_hours += exposure_hours;
                    component.failure_exposure_remaining_hours -= exposure_hours;
                    if component.failure_exposure_remaining_hours <= 0.0 {
                        component.condition = ComponentCondition::Failed;
                        component.health_pct = 0.0;
                        component.failure_count += 1;
                        self.pending_events.push(ReliabilityEvent {
                            event_kind: "component_failed".to_string(),
                            component_id: component.profile.component_id.clone(),
                            work_order_id: None,
                            detail: "deterministic seeded constant-failure-rate event".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
        self.capacity()
    }

    pub fn snapshot(&self) -> EclssReliabilitySnapshot {
        let components: Vec<_> = self
            .components
            .iter()
            .map(|component| EclssComponentSnapshot {
                component_id: component.profile.component_id.clone(),
                display_name: component.profile.display_name.clone(),
                function: component.profile.function,
                condition: component.condition,
                health_pct: component.health_pct,
                capacity_fraction: component_capacity(component),
                unit_operating_hours: component.unit_operating_hours,
                cumulative_operating_hours: component.cumulative_operating_hours,
                reference_mtbf_hours: component.profile.mtbf_hours,
                reference_mttr_hours: component.profile.mttr_hours,
                operational_k_factor: component.profile.k_factor,
                configured_failures_per_year: configured_failures_per_year(&component.profile),
                duty_cycle: component.profile.duty_cycle,
                duty_cycle_basis: component.profile.duty_cycle_basis.clone(),
                quantity_installed: component.profile.quantity_installed,
                quantity_installed_basis: component.profile.quantity_installed_basis.clone(),
                initial_spares_basis: component.profile.initial_spares_basis.clone(),
                source_table: component.profile.source_table.clone(),
                source_row: component.profile.source_row.clone(),
                source_parameter_evidence_class: "published_mads_derived_aggregate".to_string(),
                spares_remaining: component.spares_remaining,
                repair_remaining_hours: component.repair_remaining_hours,
                failure_count: component.failure_count,
                completed_repairs: component.completed_repairs,
                active_work_order: component.active_work_order.clone(),
            })
            .collect();
        let failed_count = components
            .iter()
            .filter(|component| component.condition == ComponentCondition::Failed)
            .count();
        let degraded_count = components
            .iter()
            .filter(|component| component.condition == ComponentCondition::Degraded)
            .count();
        let repairing_count = components
            .iter()
            .filter(|component| component.condition == ComponentCondition::Repairing)
            .count();
        EclssReliabilitySnapshot {
            schema_version: self.schema_version,
            profile_id: self.profile_id.clone(),
            source_document_id: self.source_document_id.clone(),
            source_report_number: self.source_report_number.clone(),
            source_parameter_evidence_class: "published_mads_derived_aggregate".to_string(),
            configuration_assumptions_present: true,
            failed_count,
            degraded_count,
            repairing_count,
            maintenance_backlog: failed_count + degraded_count + repairing_count,
            spares_remaining: components
                .iter()
                .map(|component| component.spares_remaining)
                .sum(),
            components,
        }
    }

    pub fn inject_failure(&mut self, component_id: &str) -> Result<(), String> {
        let component = self.component_mut(component_id)?;
        if component.condition == ComponentCondition::Repairing {
            return Err("cannot fail a component while its replacement is in progress".to_string());
        }
        if component.condition == ComponentCondition::Failed {
            return Err("component is already failed".to_string());
        }
        component.condition = ComponentCondition::Failed;
        component.health_pct = 0.0;
        component.failure_count += 1;
        self.pending_events.push(ReliabilityEvent {
            event_kind: "component_failure_injected".to_string(),
            component_id: component_id.to_string(),
            work_order_id: None,
            detail: "bounded validation fault".to_string(),
        });
        Ok(())
    }

    pub fn inject_degradation(
        &mut self,
        component_id: &str,
        health_pct: f64,
    ) -> Result<(), String> {
        if !health_pct.is_finite() || !(1.0..100.0).contains(&health_pct) {
            return Err("health_pct must be finite and within 1..100".to_string());
        }
        let component = self.component_mut(component_id)?;
        if matches!(
            component.condition,
            ComponentCondition::Failed | ComponentCondition::Repairing
        ) {
            return Err("only an operating component can be degraded".to_string());
        }
        component.condition = ComponentCondition::Degraded;
        component.health_pct = health_pct;
        self.pending_events.push(ReliabilityEvent {
            event_kind: "component_degradation_injected".to_string(),
            component_id: component_id.to_string(),
            work_order_id: None,
            detail: format!("bounded validation degradation to {health_pct:.1}% health"),
        });
        Ok(())
    }

    pub fn start_repair(&mut self, component_id: &str) -> Result<String, String> {
        let sequence = self.next_work_order_sequence;
        let work_order_id = format!("eclss-wo-{sequence:06}");
        let reference_mttr_hours = {
            let component = self.component_mut(component_id)?;
            if !matches!(
                component.condition,
                ComponentCondition::Failed | ComponentCondition::Degraded
            ) {
                return Err("repair requires a failed or degraded component".to_string());
            }
            if component.spares_remaining == 0 {
                return Err("no compatible spare ORU remains".to_string());
            }
            component.spares_remaining -= 1;
            component.condition = ComponentCondition::Repairing;
            component.repair_remaining_hours = component.profile.mttr_hours;
            component.active_work_order = Some(work_order_id.clone());
            component.profile.mttr_hours
        };
        self.next_work_order_sequence += 1;
        self.pending_events.push(ReliabilityEvent {
            event_kind: "repair_started".to_string(),
            component_id: component_id.to_string(),
            work_order_id: Some(work_order_id.clone()),
            detail: format!(
                "reference remove-and-replace duration {:.2} hours; logistics delay excluded",
                reference_mttr_hours
            ),
        });
        Ok(work_order_id)
    }

    pub fn add_spares(&mut self, component_id: &str, quantity: u32) -> Result<(), String> {
        if !(1..=100).contains(&quantity) {
            return Err("quantity must be within 1..=100".to_string());
        }
        let component = self.component_mut(component_id)?;
        component.spares_remaining = component
            .spares_remaining
            .checked_add(quantity)
            .filter(|total| *total <= 1_000)
            .ok_or_else(|| "component spare inventory cannot exceed 1000".to_string())?;
        self.pending_events.push(ReliabilityEvent {
            event_kind: "spare_inventory_received".to_string(),
            component_id: component_id.to_string(),
            work_order_id: None,
            detail: format!(
                "{quantity} replacement ORU(s) crossed the external logistics boundary"
            ),
        });
        Ok(())
    }

    pub fn drain_events(&mut self) -> Vec<ReliabilityEvent> {
        std::mem::take(&mut self.pending_events)
    }

    fn component_mut(&mut self, component_id: &str) -> Result<&mut EclssComponentRuntime, String> {
        self.components
            .iter_mut()
            .find(|component| component.profile.component_id == component_id)
            .ok_or_else(|| format!("unknown ECLSS component: {component_id}"))
    }

    fn capacity(&self) -> EclssCapacity {
        let mut capacity = EclssCapacity {
            oxygen_generation: 1.0,
            carbon_dioxide_removal: 1.0,
            humidity_control: 1.0,
            water_recovery: 1.0,
        };
        for component in &self.components {
            let value = component_capacity(component);
            match component.profile.function {
                EclssFunction::OxygenGeneration => capacity.oxygen_generation *= value,
                EclssFunction::CarbonDioxideRemoval => capacity.carbon_dioxide_removal *= value,
                EclssFunction::HumidityControl => capacity.humidity_control *= value,
                EclssFunction::WaterRecovery => capacity.water_recovery *= value,
            }
        }
        capacity
    }
}

fn component_capacity(component: &EclssComponentRuntime) -> f64 {
    match component.condition {
        ComponentCondition::Operational => 1.0,
        ComponentCondition::Degraded => (component.health_pct / 100.0).clamp(0.05, 1.0),
        ComponentCondition::Failed | ComponentCondition::Repairing => 0.0,
    }
}

fn configured_failures_per_year(profile: &EclssComponentProfile) -> f64 {
    profile.duty_cycle * f64::from(profile.quantity_installed) * profile.k_factor * 8_760.0
        / profile.mtbf_hours
}

fn sample_failure_exposure(rng_state: &mut u64, profile: &EclssComponentProfile) -> f64 {
    let aggregate_rate_per_operating_hour =
        profile.duty_cycle * f64::from(profile.quantity_installed) * profile.k_factor
            / profile.mtbf_hours;
    -next_open_unit(rng_state).ln() / aggregate_rate_per_operating_hour
}

fn next_open_unit(state: &mut u64) -> f64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    let value = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
    let mantissa = (value >> 11) | 1;
    (mantissa as f64) / ((1_u64 << 53) as f64)
}

pub fn load_profile() -> Result<EclssReliabilityProfileFile, String> {
    let profile: EclssReliabilityProfileFile =
        serde_json::from_str(PROFILE_JSON).map_err(|error| error.to_string())?;
    validate_profile(&profile)?;
    Ok(profile)
}

fn validate_profile(profile: &EclssReliabilityProfileFile) -> Result<(), String> {
    if profile.schema_version != 1 || profile.profile_id.trim().is_empty() {
        return Err("unsupported or empty ECLSS reliability profile identity".to_string());
    }
    if profile.source.document_id != "20250003955"
        || profile.source.report_number != "ICES-2025-127"
        || profile.source.sha256.len() != 64
        || profile.source.evidence_class != "published_mads_derived_aggregate"
    {
        return Err("ECLSS reliability source provenance is incomplete".to_string());
    }
    if profile.components.is_empty() || profile.prohibited_claims.is_empty() {
        return Err("ECLSS reliability components and claim boundaries are required".to_string());
    }
    let mut ids = std::collections::HashSet::new();
    for component in &profile.components {
        if !ids.insert(&component.component_id)
            || component.display_name.trim().is_empty()
            || component.source_table.trim().is_empty()
            || component.source_row.trim().is_empty()
            || !component.mtbf_hours.is_finite()
            || component.mtbf_hours <= 0.0
            || !component.mttr_hours.is_finite()
            || component.mttr_hours <= 0.0
            || !component.k_factor.is_finite()
            || component.k_factor <= 0.0
            || !component.duty_cycle.is_finite()
            || !(0.0..=1.0).contains(&component.duty_cycle)
            || component.quantity_installed == 0
            || component.duty_cycle_basis.trim().is_empty()
            || component.quantity_installed_basis.trim().is_empty()
            || component.initial_spares_basis.trim().is_empty()
        {
            return Err(format!(
                "invalid or duplicate ECLSS reliability component {}",
                component.component_id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_profile_is_valid_and_preserves_published_rows() {
        let profile = load_profile().unwrap();
        assert_eq!(profile.components.len(), 4);
        let oga = profile
            .components
            .iter()
            .find(|component| component.component_id == "oga_water_assembly_oru")
            .unwrap();
        assert_eq!(oga.mtbf_hours, 37_885.0);
        assert_eq!(oga.mttr_hours, 4.19);
        assert_eq!(oga.source_table, "Table 10");
    }

    #[test]
    fn seeded_exponential_sampler_matches_configured_mean() {
        let mut profiles = load_profile().unwrap().components;
        let profile = profiles.remove(0);
        let expected_mean = profile.mtbf_hours
            / (profile.duty_cycle * f64::from(profile.quantity_installed) * profile.k_factor);
        let mut rng = 42_u64;
        let sampled_mean = (0..100_000)
            .map(|_| sample_failure_exposure(&mut rng, &profile))
            .sum::<f64>()
            / 100_000.0;
        assert!((sampled_mean / expected_mean - 1.0).abs() < 0.02);
    }

    #[test]
    fn failure_repair_and_spare_use_are_cyclic_component_events() {
        let mut state = EclssReliabilityState::new_with_seed(7);
        state.inject_failure("oga_water_assembly_oru").unwrap();
        assert_eq!(state.snapshot().failed_count, 1);
        let work_order = state.start_repair("oga_water_assembly_oru").unwrap();
        assert_eq!(work_order, "eclss-wo-000001");
        state.step(4.2 * 3_600.0, true);
        let snapshot = state.snapshot();
        let oga = snapshot
            .components
            .iter()
            .find(|component| component.component_id == "oga_water_assembly_oru")
            .unwrap();
        assert_eq!(oga.condition, ComponentCondition::Operational);
        assert_eq!(oga.completed_repairs, 1);
        assert_eq!(oga.spares_remaining, 1);
        assert_eq!(snapshot.failed_count, 0);
    }

    #[test]
    fn checkpoint_round_trip_preserves_rng_and_active_repair() {
        let mut state = EclssReliabilityState::new_with_seed(99);
        state
            .inject_failure("cdra_desiccant_adsorbent_assembly")
            .unwrap();
        state
            .start_repair("cdra_desiccant_adsorbent_assembly")
            .unwrap();
        state.step(3_600.0, true);
        let encoded = serde_json::to_vec(&state).unwrap();
        let mut restored: EclssReliabilityState = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored.snapshot().repairing_count, 1);
        restored.step(16.8 * 3_600.0, true);
        assert_eq!(restored.snapshot().repairing_count, 0);
    }

    #[test]
    fn bounded_spare_delivery_keeps_component_cycles_replenishable() {
        let mut state = EclssReliabilityState::new_with_seed(3);
        state.add_spares("wpa_pump_separator", 4).unwrap();
        let wpa = state
            .snapshot()
            .components
            .into_iter()
            .find(|component| component.component_id == "wpa_pump_separator")
            .unwrap();
        assert_eq!(wpa.spares_remaining, 6);
        assert!(state.add_spares("wpa_pump_separator", 0).is_err());
        assert!(state.add_spares("wpa_pump_separator", 101).is_err());
    }
}
