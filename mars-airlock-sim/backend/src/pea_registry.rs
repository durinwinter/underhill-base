#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeaDefinition {
    pub pea_id: &'static str,
    pub pea_type: &'static str,
    pub name: &'static str,
    pub service_tag: &'static str,
    pub namespace_uri: &'static str,
    pub endpoint_path: &'static str,
    pub root_path: &'static str,
}

pub const DEFAULT_AIRLOCK_PEA_ID: &str = "AIRLOCK-PEA-001";
pub const DEFAULT_ECLSS_PEA_ID: &str = "ECLSS-PEA-001";
pub const DEFAULT_SABATIER_PEA_ID: &str = "SABATIER-PEA-001";
pub const DEFAULT_POWER_PEA_ID: &str = "POWER-PEA-001";
pub const DEFAULT_THERMAL_PEA_ID: &str = "THERMAL-PEA-001";
pub const DEFAULT_WATER_PEA_ID: &str = "WATER-PEA-001";

pub const AIRLOCK_SERVICE_TAG: &str = "AirlockService";
pub const ECLSS_SERVICE_TAG: &str = "EclssService";
pub const SABATIER_SERVICE_TAG: &str = "SabatierService";
pub const POWER_SERVICE_TAG: &str = "PowerService";
pub const THERMAL_SERVICE_TAG: &str = "ThermalService";
pub const WATER_SERVICE_TAG: &str = "WaterService";

pub const ALL_PEA_DEFINITIONS: [PeaDefinition; 6] = [
    PeaDefinition {
        pea_id: DEFAULT_AIRLOCK_PEA_ID,
        pea_type: "AIRLOCK",
        name: "Underhill Airlock",
        service_tag: AIRLOCK_SERVICE_TAG,
        namespace_uri: "urn:mars-airlock:mtp",
        endpoint_path: "/underhill/airlock",
        root_path: "Objects/MarsBase/AirlockPEA",
    },
    PeaDefinition {
        pea_id: DEFAULT_ECLSS_PEA_ID,
        pea_type: "ECLSS",
        name: "Underhill ECLSS",
        service_tag: ECLSS_SERVICE_TAG,
        namespace_uri: "urn:underhill:eclss:mtp",
        endpoint_path: "/underhill/eclss",
        root_path: "Objects/Underhill/ECLSSPEA",
    },
    PeaDefinition {
        pea_id: DEFAULT_SABATIER_PEA_ID,
        pea_type: "ISRU_SABATIER",
        name: "Underhill Sabatier",
        service_tag: SABATIER_SERVICE_TAG,
        namespace_uri: "urn:underhill:sabatier:mtp",
        endpoint_path: "/underhill/sabatier",
        root_path: "Objects/Underhill/SabatierPEA",
    },
    PeaDefinition {
        pea_id: DEFAULT_POWER_PEA_ID,
        pea_type: "POWER_MICROGRID",
        name: "Underhill Power Microgrid",
        service_tag: POWER_SERVICE_TAG,
        namespace_uri: "urn:underhill:power:mtp",
        endpoint_path: "/underhill/power",
        root_path: "Objects/Underhill/PowerPEA",
    },
    PeaDefinition {
        pea_id: DEFAULT_THERMAL_PEA_ID,
        pea_type: "THERMAL_CONTROL",
        name: "Underhill Thermal Control",
        service_tag: THERMAL_SERVICE_TAG,
        namespace_uri: "urn:underhill:thermal:mtp",
        endpoint_path: "/underhill/thermal",
        root_path: "Objects/Underhill/ThermalPEA",
    },
    PeaDefinition {
        pea_id: DEFAULT_WATER_PEA_ID,
        pea_type: "WATER_WASTE_RECOVERY",
        name: "Underhill Water and Waste Recovery",
        service_tag: WATER_SERVICE_TAG,
        namespace_uri: "urn:underhill:water:mtp",
        endpoint_path: "/underhill/water",
        root_path: "Objects/Underhill/WaterPEA",
    },
];

pub fn definition_for(pea_id: &str) -> Option<&'static PeaDefinition> {
    ALL_PEA_DEFINITIONS
        .iter()
        .find(|definition| definition.pea_id == pea_id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn registry_identity_and_protocol_fields_are_unique() {
        let mut pea_ids = HashSet::new();
        let mut service_tags = HashSet::new();
        let mut namespaces = HashSet::new();
        let mut endpoint_paths = HashSet::new();
        for definition in ALL_PEA_DEFINITIONS {
            assert!(pea_ids.insert(definition.pea_id));
            assert!(service_tags.insert(definition.service_tag));
            assert!(namespaces.insert(definition.namespace_uri));
            assert!(endpoint_paths.insert(definition.endpoint_path));
            assert!(definition.endpoint_path.starts_with("/underhill/"));
            assert!(!definition.root_path.is_empty());
        }
    }

    #[test]
    fn every_registered_pea_is_resolvable() {
        for definition in ALL_PEA_DEFINITIONS {
            assert_eq!(definition_for(definition.pea_id), Some(&definition));
        }
    }
}
