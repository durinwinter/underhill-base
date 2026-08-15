use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

pub const CATALOG_SCHEMA_VERSION: u32 = 1;
pub const FULL_BASE_TAG_COUNT: usize = 110_000;

const MODEL_BACKED_TAG_IDS: &[&str] = &[
    "underhill.v1.eclss.00000.pressure",
    "underhill.v1.eclss.00000.oxygen",
    "underhill.v1.eclss.00000.carbon_dioxide",
    "underhill.v1.eclss.00000.humidity",
    "underhill.v1.eclss.00000.alarm_active",
    "underhill.v1.power.00000.state_of_charge",
    "underhill.v1.power.00000.protection_state",
    "underhill.v1.water_waste.00000.true_value",
    "underhill.v1.water_waste.00000.residual",
    "underhill.v1.water_waste.00000.alarm_active",
    "underhill.v1.water_waste.00000.quality_code",
    "underhill.v1.water_waste.00000.health_state",
    "underhill.v1.safety_structure.00000.pressure",
    "underhill.v1.safety_structure.00000.sensor_residual",
    "underhill.v1.safety_structure.00000.alarm_active",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TagRole {
    InternalTruth,
    Sensed,
    Derived,
    Commanded,
    Diagnostic,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TagValueType {
    Float64,
    Boolean,
    UInt64,
    String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CanonicalTag {
    pub schema_version: u32,
    pub tag_id: String,
    pub owner_pea: String,
    pub subsystem_family: String,
    pub equipment_path: String,
    pub signal: String,
    pub role: TagRole,
    pub value_type: TagValueType,
    pub engineering_unit: String,
    pub range_min: Option<f64>,
    pub range_max: Option<f64>,
    pub internal_cadence_hz: f64,
    pub sensing_cadence_hz: f64,
    pub publication_cadence_hz: f64,
    pub publication_class: String,
    pub deadband: Option<f64>,
    pub criticality: String,
    pub retention_class: String,
    pub opcua_subscription_class: String,
    pub quality_states: Vec<String>,
    pub activation_state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogStats {
    pub schema_version: u32,
    pub canonical_tags: usize,
    pub subsystem_counts: BTreeMap<String, usize>,
    pub owner_pea_counts: BTreeMap<String, usize>,
    pub publication_class_counts: BTreeMap<String, usize>,
    pub activation_state_counts: BTreeMap<String, usize>,
    pub nominal_publications_per_second: f64,
    pub naive_ten_hz_publications_per_second: usize,
    pub estimated_metadata_bytes: usize,
}

#[derive(Debug)]
pub struct TagCatalog {
    tags: Vec<CanonicalTag>,
    stats: CatalogStats,
}

#[derive(Clone, Copy)]
struct FamilySpec {
    slug: &'static str,
    owner_pea: &'static str,
    equipment_prefix: &'static str,
    budget: usize,
    signals: &'static [SignalSpec],
}

#[derive(Clone, Copy)]
struct SignalSpec {
    name: &'static str,
    role: TagRoleSpec,
    value_type: ValueTypeSpec,
    unit: &'static str,
    range_min: Option<f64>,
    range_max: Option<f64>,
    internal_hz: f64,
    sensing_hz: f64,
    publish_hz: f64,
    deadband: Option<f64>,
    criticality: &'static str,
    retention: &'static str,
}

#[derive(Clone, Copy)]
enum TagRoleSpec {
    Truth,
    Sensed,
    Derived,
    Commanded,
    Diagnostic,
}

#[derive(Clone, Copy)]
enum ValueTypeSpec {
    Float,
    Bool,
    UInt,
    Text,
}

const PROCESS_SIGNALS: &[SignalSpec] = &[
    signal(
        "true_value",
        TagRoleSpec::Truth,
        ValueTypeSpec::Float,
        "normalized",
        Some(0.0),
        Some(1.0),
        20.0,
        0.0,
        0.0,
        None,
        "supporting",
        "forensic",
    ),
    signal(
        "measured_value",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "normalized",
        Some(0.0),
        Some(1.0),
        20.0,
        5.0,
        1.0,
        Some(0.002),
        "operational",
        "hot_30d",
    ),
    signal(
        "setpoint",
        TagRoleSpec::Commanded,
        ValueTypeSpec::Float,
        "normalized",
        Some(0.0),
        Some(1.0),
        20.0,
        1.0,
        1.0,
        Some(0.001),
        "operational",
        "hot_30d",
    ),
    signal(
        "residual",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Float,
        "normalized",
        Some(-1.0),
        Some(1.0),
        20.0,
        5.0,
        1.0,
        Some(0.002),
        "operational",
        "warm_1y",
    ),
    signal(
        "alarm_active",
        TagRoleSpec::Derived,
        ValueTypeSpec::Bool,
        "bool",
        None,
        None,
        20.0,
        5.0,
        0.0,
        None,
        "safety",
        "events_permanent",
    ),
    signal(
        "quality_code",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::UInt,
        "code",
        Some(0.0),
        Some(255.0),
        5.0,
        1.0,
        0.2,
        None,
        "operational",
        "warm_1y",
    ),
    signal(
        "health_state",
        TagRoleSpec::Derived,
        ValueTypeSpec::Text,
        "state",
        None,
        None,
        1.0,
        1.0,
        0.1,
        None,
        "operational",
        "warm_1y",
    ),
    signal(
        "runtime_hours",
        TagRoleSpec::Derived,
        ValueTypeSpec::Float,
        "h",
        Some(0.0),
        None,
        1.0,
        0.1,
        1.0 / 60.0,
        Some(0.01),
        "supporting",
        "cold_10y",
    ),
];

const POWER_SIGNALS: &[SignalSpec] = &[
    signal(
        "cell_voltage",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "V",
        Some(0.0),
        Some(5.0),
        100.0,
        10.0,
        1.0,
        Some(0.005),
        "safety",
        "hot_30d",
    ),
    signal(
        "cell_temperature",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "degC",
        Some(-80.0),
        Some(90.0),
        20.0,
        2.0,
        0.5,
        Some(0.1),
        "safety",
        "hot_30d",
    ),
    signal(
        "branch_current",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "A",
        Some(-500.0),
        Some(500.0),
        100.0,
        10.0,
        2.0,
        Some(0.1),
        "safety",
        "hot_30d",
    ),
    signal(
        "state_of_charge",
        TagRoleSpec::Derived,
        ValueTypeSpec::Float,
        "%",
        Some(0.0),
        Some(100.0),
        10.0,
        1.0,
        0.2,
        Some(0.05),
        "operational",
        "warm_1y",
    ),
    signal(
        "breaker_closed",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Bool,
        "bool",
        None,
        None,
        100.0,
        20.0,
        0.0,
        None,
        "safety",
        "events_permanent",
    ),
    signal(
        "trip_command",
        TagRoleSpec::Commanded,
        ValueTypeSpec::Bool,
        "bool",
        None,
        None,
        100.0,
        20.0,
        0.0,
        None,
        "safety",
        "events_permanent",
    ),
    signal(
        "insulation_resistance",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "ohm",
        Some(0.0),
        None,
        1.0,
        0.2,
        1.0 / 60.0,
        Some(100.0),
        "safety",
        "cold_10y",
    ),
    signal(
        "protection_state",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Text,
        "state",
        None,
        None,
        20.0,
        5.0,
        0.0,
        None,
        "safety",
        "events_permanent",
    ),
];

const ATMOSPHERE_SIGNALS: &[SignalSpec] = &[
    signal(
        "pressure",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "kPa",
        Some(0.0),
        Some(120.0),
        20.0,
        5.0,
        1.0,
        Some(0.01),
        "safety",
        "hot_30d",
    ),
    signal(
        "oxygen",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "%",
        Some(0.0),
        Some(25.0),
        5.0,
        1.0,
        0.2,
        Some(0.01),
        "safety",
        "warm_1y",
    ),
    signal(
        "carbon_dioxide",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "ppm",
        Some(0.0),
        Some(10000.0),
        5.0,
        1.0,
        0.2,
        Some(5.0),
        "safety",
        "warm_1y",
    ),
    signal(
        "humidity",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "%RH",
        Some(0.0),
        Some(100.0),
        2.0,
        1.0,
        0.2,
        Some(0.1),
        "operational",
        "warm_1y",
    ),
    signal(
        "flow_command",
        TagRoleSpec::Commanded,
        ValueTypeSpec::Float,
        "%",
        Some(0.0),
        Some(100.0),
        20.0,
        5.0,
        1.0,
        Some(0.1),
        "operational",
        "hot_30d",
    ),
    signal(
        "sensor_residual",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Float,
        "sigma",
        Some(-10.0),
        Some(10.0),
        5.0,
        1.0,
        0.2,
        Some(0.05),
        "operational",
        "warm_1y",
    ),
    signal(
        "alarm_active",
        TagRoleSpec::Derived,
        ValueTypeSpec::Bool,
        "bool",
        None,
        None,
        20.0,
        5.0,
        0.0,
        None,
        "safety",
        "events_permanent",
    ),
    signal(
        "sample_age",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Float,
        "s",
        Some(0.0),
        None,
        1.0,
        1.0,
        0.2,
        Some(0.1),
        "operational",
        "hot_30d",
    ),
];

const INVENTORY_SIGNALS: &[SignalSpec] = &[
    signal(
        "quantity",
        TagRoleSpec::Sensed,
        ValueTypeSpec::Float,
        "kg",
        Some(0.0),
        None,
        1.0,
        0.2,
        1.0 / 60.0,
        Some(0.01),
        "operational",
        "cold_10y",
    ),
    signal(
        "capacity",
        TagRoleSpec::Truth,
        ValueTypeSpec::Float,
        "kg",
        Some(0.0),
        None,
        0.1,
        0.0,
        0.0,
        None,
        "supporting",
        "configuration",
    ),
    signal(
        "reserved_quantity",
        TagRoleSpec::Derived,
        ValueTypeSpec::Float,
        "kg",
        Some(0.0),
        None,
        0.2,
        0.1,
        1.0 / 60.0,
        Some(0.01),
        "operational",
        "cold_10y",
    ),
    signal(
        "reorder_point",
        TagRoleSpec::Commanded,
        ValueTypeSpec::Float,
        "kg",
        Some(0.0),
        None,
        0.1,
        0.1,
        1.0 / 300.0,
        Some(0.01),
        "supporting",
        "configuration",
    ),
    signal(
        "expiry_sol",
        TagRoleSpec::Derived,
        ValueTypeSpec::UInt,
        "sol",
        Some(0.0),
        None,
        0.1,
        0.1,
        1.0 / 300.0,
        None,
        "supporting",
        "cold_10y",
    ),
    signal(
        "location_state",
        TagRoleSpec::Derived,
        ValueTypeSpec::Text,
        "state",
        None,
        None,
        0.2,
        0.2,
        0.0,
        None,
        "operational",
        "events_permanent",
    ),
    signal(
        "quality_hold",
        TagRoleSpec::Commanded,
        ValueTypeSpec::Bool,
        "bool",
        None,
        None,
        1.0,
        1.0,
        0.0,
        None,
        "safety",
        "events_permanent",
    ),
    signal(
        "reconciliation_error",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Float,
        "kg",
        None,
        None,
        0.2,
        0.2,
        1.0 / 60.0,
        Some(0.001),
        "operational",
        "warm_1y",
    ),
];

const DIAGNOSTIC_SIGNALS: &[SignalSpec] = &[
    signal(
        "mass_balance_error",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Float,
        "kg/s",
        None,
        None,
        20.0,
        5.0,
        1.0,
        Some(0.0001),
        "safety",
        "warm_1y",
    ),
    signal(
        "energy_balance_error",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Float,
        "kW",
        None,
        None,
        20.0,
        5.0,
        1.0,
        Some(0.001),
        "safety",
        "warm_1y",
    ),
    signal(
        "model_confidence",
        TagRoleSpec::Derived,
        ValueTypeSpec::Float,
        "ratio",
        Some(0.0),
        Some(1.0),
        1.0,
        1.0,
        0.2,
        Some(0.001),
        "operational",
        "warm_1y",
    ),
    signal(
        "forecast_margin",
        TagRoleSpec::Derived,
        ValueTypeSpec::Float,
        "h",
        None,
        None,
        0.2,
        0.2,
        1.0 / 60.0,
        Some(0.1),
        "operational",
        "cold_10y",
    ),
    signal(
        "fault_probability",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Float,
        "ratio",
        Some(0.0),
        Some(1.0),
        1.0,
        1.0,
        0.2,
        Some(0.001),
        "operational",
        "warm_1y",
    ),
    signal(
        "quality_code",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::UInt,
        "code",
        Some(0.0),
        Some(255.0),
        5.0,
        1.0,
        0.2,
        None,
        "operational",
        "warm_1y",
    ),
    signal(
        "invariant_violation",
        TagRoleSpec::Derived,
        ValueTypeSpec::Bool,
        "bool",
        None,
        None,
        20.0,
        5.0,
        0.0,
        None,
        "safety",
        "events_permanent",
    ),
    signal(
        "evidence_reference",
        TagRoleSpec::Diagnostic,
        ValueTypeSpec::Text,
        "uri",
        None,
        None,
        0.1,
        0.1,
        0.0,
        None,
        "supporting",
        "events_permanent",
    ),
];

const fn signal(
    name: &'static str,
    role: TagRoleSpec,
    value_type: ValueTypeSpec,
    unit: &'static str,
    range_min: Option<f64>,
    range_max: Option<f64>,
    internal_hz: f64,
    sensing_hz: f64,
    publish_hz: f64,
    deadband: Option<f64>,
    criticality: &'static str,
    retention: &'static str,
) -> SignalSpec {
    SignalSpec {
        name,
        role,
        value_type,
        unit,
        range_min,
        range_max,
        internal_hz,
        sensing_hz,
        publish_hz,
        deadband,
        criticality,
        retention,
    }
}

const FAMILIES: &[FamilySpec] = &[
    FamilySpec {
        slug: "environment",
        owner_pea: "ENVIRONMENT-PEA-001",
        equipment_prefix: "site_sensor",
        budget: 5_000,
        signals: PROCESS_SIGNALS,
    },
    FamilySpec {
        slug: "power",
        owner_pea: "POWER-PEA-001",
        equipment_prefix: "electrical_asset",
        budget: 14_000,
        signals: POWER_SIGNALS,
    },
    FamilySpec {
        slug: "thermal",
        owner_pea: "THERMAL-PEA-001",
        equipment_prefix: "thermal_branch",
        budget: 10_000,
        signals: PROCESS_SIGNALS,
    },
    FamilySpec {
        slug: "eclss",
        owner_pea: "ECLSS-PEA-001",
        equipment_prefix: "atmosphere_asset",
        budget: 14_000,
        signals: ATMOSPHERE_SIGNALS,
    },
    FamilySpec {
        slug: "water_waste",
        owner_pea: "WATER-PEA-001",
        equipment_prefix: "treatment_asset",
        budget: 10_000,
        signals: PROCESS_SIGNALS,
    },
    FamilySpec {
        slug: "safety_structure",
        owner_pea: "SAFETY-PEA-001",
        equipment_prefix: "safety_zone",
        budget: 8_000,
        signals: ATMOSPHERE_SIGNALS,
    },
    FamilySpec {
        slug: "gas_sabatier",
        owner_pea: "SABATIER-PEA-001",
        equipment_prefix: "gas_asset",
        budget: 7_000,
        signals: PROCESS_SIGNALS,
    },
    FamilySpec {
        slug: "isru_manufacturing",
        owner_pea: "ISRU-PEA-001",
        equipment_prefix: "process_asset",
        budget: 9_000,
        signals: PROCESS_SIGNALS,
    },
    FamilySpec {
        slug: "agriculture",
        owner_pea: "AGRICULTURE-PEA-001",
        equipment_prefix: "growth_asset",
        budget: 8_000,
        signals: ATMOSPHERE_SIGNALS,
    },
    FamilySpec {
        slug: "robotics_logistics",
        owner_pea: "ROBOTICS-PEA-001",
        equipment_prefix: "maintainable_asset",
        budget: 8_000,
        signals: INVENTORY_SIGNALS,
    },
    FamilySpec {
        slug: "eva_mobility",
        owner_pea: "AIRLOCK-PEA-001",
        equipment_prefix: "eva_asset",
        budget: 4_000,
        signals: INVENTORY_SIGNALS,
    },
    FamilySpec {
        slug: "comms_compute_cyber",
        owner_pea: "COMPUTE-PEA-001",
        equipment_prefix: "compute_asset",
        budget: 5_000,
        signals: DIAGNOSTIC_SIGNALS,
    },
    FamilySpec {
        slug: "cross_plant_diagnostics",
        owner_pea: "PLANT-DIAGNOSTICS-PEA-001",
        equipment_prefix: "ledger",
        budget: 8_000,
        signals: DIAGNOSTIC_SIGNALS,
    },
];

impl TagCatalog {
    pub fn full_base() -> Self {
        let mut tags = Vec::with_capacity(FULL_BASE_TAG_COUNT);
        for family in FAMILIES {
            for ordinal in 0..family.budget {
                let template = family.signals[ordinal % family.signals.len()];
                let equipment_index = ordinal / family.signals.len();
                let publication_class = publication_class(template.publish_hz);
                let tag_id = format!(
                    "underhill.v1.{}.{:05}.{}",
                    family.slug, equipment_index, template.name
                );
                let activation_state = if MODEL_BACKED_TAG_IDS.contains(&tag_id.as_str()) {
                    "model_backed"
                } else {
                    "planned"
                };
                tags.push(CanonicalTag {
                    schema_version: CATALOG_SCHEMA_VERSION,
                    tag_id,
                    owner_pea: family.owner_pea.to_string(),
                    subsystem_family: family.slug.to_string(),
                    equipment_path: format!(
                        "Objects/Underhill/{}/{}/{:05}",
                        family.owner_pea, family.equipment_prefix, equipment_index
                    ),
                    signal: template.name.to_string(),
                    role: role(template.role),
                    value_type: value_type(template.value_type),
                    engineering_unit: template.unit.to_string(),
                    range_min: template.range_min,
                    range_max: template.range_max,
                    internal_cadence_hz: template.internal_hz,
                    sensing_cadence_hz: template.sensing_hz,
                    publication_cadence_hz: template.publish_hz,
                    publication_class: publication_class.to_string(),
                    deadband: template.deadband,
                    criticality: template.criticality.to_string(),
                    retention_class: template.retention.to_string(),
                    opcua_subscription_class: opcua_class(template).to_string(),
                    quality_states: vec![
                        "Good".to_string(),
                        "Uncertain".to_string(),
                        "Stale".to_string(),
                        "Bad".to_string(),
                    ],
                    activation_state: activation_state.to_string(),
                });
            }
        }
        let stats = calculate_stats(&tags);
        Self { tags, stats }
    }

    pub fn tags(&self) -> &[CanonicalTag] {
        &self.tags
    }

    pub fn stats(&self) -> &CatalogStats {
        &self.stats
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.tags.len() != FULL_BASE_TAG_COUNT {
            return Err(format!(
                "expected {FULL_BASE_TAG_COUNT} tags, got {}",
                self.tags.len()
            ));
        }
        let mut ids = HashSet::with_capacity(self.tags.len());
        for tag in &self.tags {
            if !ids.insert(&tag.tag_id) {
                return Err(format!("duplicate tag id {}", tag.tag_id));
            }
            if tag.owner_pea.is_empty()
                || tag.equipment_path.is_empty()
                || tag.signal.is_empty()
                || tag.internal_cadence_hz < tag.sensing_cadence_hz
                || tag.sensing_cadence_hz < tag.publication_cadence_hz
            {
                return Err(format!("invalid tag metadata for {}", tag.tag_id));
            }
            if tag
                .range_min
                .zip(tag.range_max)
                .is_some_and(|(min, max)| min >= max)
            {
                return Err(format!("invalid range for {}", tag.tag_id));
            }
        }
        Ok(())
    }
}

fn calculate_stats(tags: &[CanonicalTag]) -> CatalogStats {
    let mut subsystem_counts = BTreeMap::new();
    let mut owner_pea_counts = BTreeMap::new();
    let mut publication_class_counts = BTreeMap::new();
    let mut activation_state_counts = BTreeMap::new();
    let mut nominal_publications_per_second = 0.0;
    let mut estimated_metadata_bytes = 0;
    for tag in tags {
        *subsystem_counts
            .entry(tag.subsystem_family.clone())
            .or_insert(0) += 1;
        *owner_pea_counts.entry(tag.owner_pea.clone()).or_insert(0) += 1;
        *publication_class_counts
            .entry(tag.publication_class.clone())
            .or_insert(0) += 1;
        *activation_state_counts
            .entry(tag.activation_state.clone())
            .or_insert(0) += 1;
        nominal_publications_per_second += tag.publication_cadence_hz;
        estimated_metadata_bytes += tag.tag_id.len()
            + tag.owner_pea.len()
            + tag.subsystem_family.len()
            + tag.equipment_path.len()
            + tag.signal.len()
            + tag.engineering_unit.len()
            + tag.publication_class.len()
            + tag.criticality.len()
            + tag.retention_class.len()
            + tag.opcua_subscription_class.len()
            + tag.activation_state.len();
    }
    CatalogStats {
        schema_version: CATALOG_SCHEMA_VERSION,
        canonical_tags: tags.len(),
        subsystem_counts,
        owner_pea_counts,
        publication_class_counts,
        activation_state_counts,
        nominal_publications_per_second,
        naive_ten_hz_publications_per_second: tags.len() * 10,
        estimated_metadata_bytes,
    }
}

fn publication_class(hz: f64) -> &'static str {
    if hz == 0.0 {
        "event_only"
    } else if hz >= 2.0 {
        "fast"
    } else if hz >= 0.2 {
        "normal"
    } else if hz >= 1.0 / 60.0 {
        "slow"
    } else {
        "archive"
    }
}

fn opcua_class(template: SignalSpec) -> &'static str {
    if template.criticality == "safety" {
        "priority_protection"
    } else if template.publish_hz == 0.0 {
        "event"
    } else if template.publish_hz >= 1.0 {
        "operational_fast"
    } else {
        "operational_slow"
    }
}

fn role(role: TagRoleSpec) -> TagRole {
    match role {
        TagRoleSpec::Truth => TagRole::InternalTruth,
        TagRoleSpec::Sensed => TagRole::Sensed,
        TagRoleSpec::Derived => TagRole::Derived,
        TagRoleSpec::Commanded => TagRole::Commanded,
        TagRoleSpec::Diagnostic => TagRole::Diagnostic,
    }
}

fn value_type(value_type: ValueTypeSpec) -> TagValueType {
    match value_type {
        ValueTypeSpec::Float => TagValueType::Float64,
        ValueTypeSpec::Bool => TagValueType::Boolean,
        ValueTypeSpec::UInt => TagValueType::UInt64,
        ValueTypeSpec::Text => TagValueType::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_catalog_matches_the_planned_base_budget_without_collisions() {
        let catalog = TagCatalog::full_base();
        catalog.validate().unwrap();
        assert_eq!(catalog.stats.canonical_tags, 110_000);
        assert_eq!(catalog.stats.subsystem_counts["power"], 14_000);
        assert_eq!(catalog.stats.subsystem_counts["eclss"], 14_000);
        assert_eq!(
            catalog.stats.subsystem_counts["cross_plant_diagnostics"],
            8_000
        );
        assert_eq!(catalog.stats.activation_state_counts["planned"], 109_985);
        assert_eq!(catalog.stats.activation_state_counts["model_backed"], 15);
    }

    #[test]
    fn cadence_budget_is_materially_below_naive_ten_hz_broadcast() {
        let catalog = TagCatalog::full_base();
        assert!(catalog.stats.nominal_publications_per_second > 10_000.0);
        assert!(catalog.stats.nominal_publications_per_second < 300_000.0);
        assert_eq!(
            catalog.stats.naive_ten_hz_publications_per_second,
            1_100_000
        );
        assert!(catalog.stats.publication_class_counts["event_only"] > 0);
        assert!(catalog.stats.publication_class_counts["slow"] > 0);
    }

    #[test]
    fn generated_ids_are_stable_across_catalog_instances() {
        let first = TagCatalog::full_base();
        let second = TagCatalog::full_base();
        for index in [0, 4_999, 5_000, 18_999, 109_999] {
            assert_eq!(first.tags[index].tag_id, second.tags[index].tag_id);
        }
    }
}
