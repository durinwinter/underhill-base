use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::eclss_reliability::{
    EclssReliabilitySnapshot, EclssReliabilityState, EclssRepairRequest, ReliabilityEvent,
};

#[derive(Debug, Clone, Serialize)]
pub struct EclssSnapshot {
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub cabin_pressure_kpa: f64,
    pub o2_percent: f64,
    pub co2_ppm: f64,
    pub humidity_pct: f64,
    pub water_recovery_pct: f64,
    pub co2_capture_kgph: f64,
    pub o2_generation_kgph: f64,
    pub power_kw: f64,
    pub alarm_high_co2: bool,
    pub alarm_low_o2: bool,
    pub alarm_maintenance_required: bool,
    pub reliability: EclssReliabilitySnapshot,
    pub co2_scrubber_running: bool,
    pub electrolyzer_running: bool,
    pub water_processor_running: bool,
    pub branch_isolated: bool,
    pub safe_haven_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SabatierSnapshot {
    pub timestamp_ms: u64,
    pub reactor_temp_c: f64,
    pub reactor_pressure_bar: f64,
    pub co2_feed_kgph: f64,
    pub h2_feed_kgph: f64,
    pub conversion_efficiency_pct: f64,
    pub methane_production_kgph: f64,
    pub water_production_kgph: f64,
    pub catalyst_health_pct: f64,
    pub power_kw: f64,
    pub alarm_reactor_temp: bool,
    pub methanation_enabled: bool,
    pub feed_conditioning_enabled: bool,
    pub hydrogen_recovery_enabled: bool,
    pub catalyst_regen_active: bool,
    pub emergency_vent_active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PowerSnapshot {
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub solar_irradiance_w_m2: f64,
    pub dust_opacity: f64,
    pub solar_available_kw: f64,
    pub fission_available_kw: f64,
    pub generation_kw: f64,
    pub requested_load_kw: f64,
    pub served_load_kw: f64,
    pub critical_load_kw: f64,
    pub flexible_load_kw: f64,
    pub battery_power_kw: f64,
    pub battery_energy_kwh: f64,
    pub battery_soc_pct: f64,
    pub bus_voltage_v: f64,
    pub unmet_load_kw: f64,
    pub curtailed_generation_kw: f64,
    pub load_shed_active: bool,
    pub alarm_battery_low: bool,
    pub alarm_bus_undervoltage: bool,
    pub cumulative_generated_kwh: f64,
    pub cumulative_requested_kwh: f64,
    pub cumulative_served_kwh: f64,
    pub cumulative_unserved_kwh: f64,
    pub instantaneous_balance_error_kw: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThermalSnapshot {
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub mars_ambient_temp_c: f64,
    pub habitat_temp_c: f64,
    pub coolant_supply_temp_c: f64,
    pub coolant_return_temp_c: f64,
    pub coolant_flow_kg_s: f64,
    pub radiator_deployment_pct: f64,
    pub equipment_heat_load_kw: f64,
    pub heater_power_kw: f64,
    pub heat_rejection_kw: f64,
    pub pump_electric_power_kw: f64,
    pub stored_thermal_energy_mj: f64,
    pub cumulative_heat_load_kwh: f64,
    pub cumulative_heat_rejected_kwh: f64,
    pub instantaneous_balance_error_kw: f64,
    pub alarm_habitat_hot: bool,
    pub alarm_habitat_cold: bool,
    pub alarm_coolant_hot: bool,
    pub cooling_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaterSnapshot {
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub potable_water_kg: f64,
    pub potable_capacity_kg: f64,
    pub wastewater_kg: f64,
    pub wastewater_capacity_kg: f64,
    pub brine_kg: f64,
    pub brine_capacity_kg: f64,
    pub discharged_water_kg: f64,
    pub crew_demand_kgph: f64,
    pub crew_water_served_kgph: f64,
    pub unmet_crew_water_kgph: f64,
    pub external_water_input_kgph: f64,
    pub treatment_feed_kgph: f64,
    pub reclaimed_water_kgph: f64,
    pub brine_production_kgph: f64,
    pub recovery_efficiency_pct: f64,
    pub potable_conductivity_us_cm: f64,
    pub potable_toc_mg_l: f64,
    pub microbial_cfu_ml: f64,
    pub treatment_power_kw: f64,
    pub cumulative_external_input_kg: f64,
    pub cumulative_crew_consumption_kg: f64,
    pub cumulative_reclaimed_kg: f64,
    pub instantaneous_balance_error_kgph: f64,
    pub treatment_available: bool,
    pub alarm_potable_low: bool,
    pub alarm_wastewater_high: bool,
    pub alarm_brine_high: bool,
    pub alarm_water_quality: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafetySnapshot {
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub habitat_air_mass_kg: f64,
    pub habitat_volume_m3: f64,
    pub habitat_pressure_kpa: f64,
    pub exterior_pressure_kpa: f64,
    pub pressure_decay_kpa_per_min: f64,
    pub leak_rate_kgph: f64,
    pub makeup_gas_kgph: f64,
    pub cumulative_leaked_air_kg: f64,
    pub cumulative_makeup_air_kg: f64,
    pub instantaneous_mass_balance_error_kgph: f64,
    pub smoke_ppm: f64,
    pub carbon_monoxide_ppm: f64,
    pub fire_heat_release_kw: f64,
    pub suppression_agent_kg: f64,
    pub suppression_flow_kgph: f64,
    pub habitat_isolated: bool,
    pub pressure_shell_strain_microstrain: f64,
    pub structural_integrity_pct: f64,
    pub external_radiation_msv_h: f64,
    pub internal_radiation_msv_h: f64,
    pub cumulative_internal_dose_msv: f64,
    pub safety_power_kw: f64,
    pub monitoring_available: bool,
    pub alarm_low_pressure: bool,
    pub alarm_rapid_decompression: bool,
    pub alarm_fire: bool,
    pub alarm_toxic_gas: bool,
    pub alarm_radiation: bool,
    pub alarm_structural: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSimulation {
    sim_time_sec: f64,
    solar_array_capacity_kw: f64,
    solar_array_area_m2: f64,
    solar_array_health_pct: f64,
    dust_opacity: f64,
    fission_capacity_kw: f64,
    battery_capacity_kwh: f64,
    battery_energy_kwh: f64,
    battery_max_charge_kw: f64,
    battery_max_discharge_kw: f64,
    battery_charge_efficiency: f64,
    battery_discharge_efficiency: f64,
    bus_nominal_voltage_v: f64,
    load_shed_active: bool,
    solar_irradiance_w_m2: f64,
    solar_available_kw: f64,
    fission_available_kw: f64,
    generation_kw: f64,
    requested_load_kw: f64,
    served_load_kw: f64,
    critical_load_kw: f64,
    flexible_load_kw: f64,
    battery_power_kw: f64,
    bus_voltage_v: f64,
    unmet_load_kw: f64,
    curtailed_generation_kw: f64,
    cumulative_generated_kwh: f64,
    cumulative_requested_kwh: f64,
    cumulative_served_kwh: f64,
    cumulative_unserved_kwh: f64,
    instantaneous_balance_error_kw: f64,
}

impl PowerSimulation {
    pub fn new() -> Self {
        let battery_capacity_kwh = 500.0;
        Self {
            sim_time_sec: 0.0,
            solar_array_capacity_kw: 120.0,
            solar_array_area_m2: 260.0,
            solar_array_health_pct: 100.0,
            dust_opacity: 0.20,
            fission_capacity_kw: 40.0,
            battery_capacity_kwh,
            battery_energy_kwh: battery_capacity_kwh * 0.80,
            battery_max_charge_kw: 100.0,
            battery_max_discharge_kw: 100.0,
            battery_charge_efficiency: 0.95,
            battery_discharge_efficiency: 0.95,
            bus_nominal_voltage_v: 400.0,
            load_shed_active: false,
            solar_irradiance_w_m2: 0.0,
            solar_available_kw: 0.0,
            fission_available_kw: 0.0,
            generation_kw: 0.0,
            requested_load_kw: 0.0,
            served_load_kw: 0.0,
            critical_load_kw: 0.0,
            flexible_load_kw: 0.0,
            battery_power_kw: 0.0,
            bus_voltage_v: 400.0,
            unmet_load_kw: 0.0,
            curtailed_generation_kw: 0.0,
            cumulative_generated_kwh: 0.0,
            cumulative_requested_kwh: 0.0,
            cumulative_served_kwh: 0.0,
            cumulative_unserved_kwh: 0.0,
            instantaneous_balance_error_kw: 0.0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        surface_solar_irradiance_w_m2: f64,
        dust_optical_depth: f64,
        eclss_load_kw: f64,
        thermal_load_kw: f64,
        water_load_kw: f64,
        safety_load_kw: f64,
        maintenance_load_kw: f64,
        sabatier_load_kw: f64,
        airlock_load_kw: f64,
    ) -> PowerSnapshot {
        self.sim_time_sec += dt_sec;
        self.solar_irradiance_w_m2 = surface_solar_irradiance_w_m2.max(0.0);
        self.dust_opacity = dust_optical_depth.max(0.0);
        let area_limited_kw = self.solar_irradiance_w_m2 * self.solar_array_area_m2 / 1_000.0;
        self.solar_available_kw = if running {
            area_limited_kw.min(self.solar_array_capacity_kw)
                * (self.solar_array_health_pct / 100.0)
        } else {
            0.0
        };
        self.fission_available_kw = if running {
            self.fission_capacity_kw
        } else {
            0.0
        };
        self.generation_kw = self.solar_available_kw + self.fission_available_kw;

        self.critical_load_kw = 18.0
            + eclss_load_kw.max(0.0)
            + thermal_load_kw.max(0.0)
            + water_load_kw.max(0.0)
            + safety_load_kw.max(0.0)
            + maintenance_load_kw.max(0.0)
            + airlock_load_kw.max(0.0);
        self.flexible_load_kw = 8.0 + sabatier_load_kw.max(0.0);
        let battery_soc_pct = self.battery_soc_pct();
        self.load_shed_active = battery_soc_pct < 12.0 || self.bus_voltage_v < 360.0;
        self.requested_load_kw = self.critical_load_kw
            + if self.load_shed_active {
                0.0
            } else {
                self.flexible_load_kw
            };

        let dt_hours = dt_sec / 3_600.0;
        let net_generation_kw = self.generation_kw - self.requested_load_kw;
        let mut charge_input_kw = 0.0;
        let mut discharge_output_kw = 0.0;
        self.unmet_load_kw = 0.0;
        self.curtailed_generation_kw = 0.0;

        if running && net_generation_kw >= 0.0 {
            let storage_room_kwh = (self.battery_capacity_kwh - self.battery_energy_kwh).max(0.0);
            let room_limited_kw = if dt_hours > 0.0 {
                storage_room_kwh / (dt_hours * self.battery_charge_efficiency)
            } else {
                0.0
            };
            charge_input_kw = net_generation_kw
                .min(self.battery_max_charge_kw)
                .min(room_limited_kw);
            self.battery_energy_kwh += charge_input_kw * self.battery_charge_efficiency * dt_hours;
            self.curtailed_generation_kw = (net_generation_kw - charge_input_kw).max(0.0);
        } else if running {
            let deficit_kw = -net_generation_kw;
            let energy_limited_kw = if dt_hours > 0.0 {
                self.battery_energy_kwh * self.battery_discharge_efficiency / dt_hours
            } else {
                0.0
            };
            discharge_output_kw = deficit_kw
                .min(self.battery_max_discharge_kw)
                .min(energy_limited_kw);
            self.battery_energy_kwh -=
                discharge_output_kw / self.battery_discharge_efficiency * dt_hours;
            self.unmet_load_kw = (deficit_kw - discharge_output_kw).max(0.0);
        } else {
            self.unmet_load_kw = self.requested_load_kw;
        }

        self.battery_energy_kwh = self
            .battery_energy_kwh
            .clamp(0.0, self.battery_capacity_kwh);
        self.battery_power_kw = charge_input_kw - discharge_output_kw;
        self.served_load_kw = (self.requested_load_kw - self.unmet_load_kw).max(0.0);
        let service_fraction = if self.requested_load_kw > 0.0 {
            self.served_load_kw / self.requested_load_kw
        } else {
            1.0
        };
        self.bus_voltage_v = self.bus_nominal_voltage_v * service_fraction.clamp(0.0, 1.0);
        self.instantaneous_balance_error_kw = self.generation_kw + discharge_output_kw
            - self.served_load_kw
            - charge_input_kw
            - self.curtailed_generation_kw;

        self.cumulative_generated_kwh += self.generation_kw * dt_hours;
        self.cumulative_requested_kwh += self.requested_load_kw * dt_hours;
        self.cumulative_served_kwh += self.served_load_kw * dt_hours;
        self.cumulative_unserved_kwh += self.unmet_load_kw * dt_hours;
        self.snapshot()
    }

    pub fn snapshot(&self) -> PowerSnapshot {
        let battery_soc_pct = self.battery_soc_pct();
        PowerSnapshot {
            timestamp_ms: now_ms(),
            sim_time_sec: self.sim_time_sec,
            solar_irradiance_w_m2: self.solar_irradiance_w_m2,
            dust_opacity: self.dust_opacity,
            solar_available_kw: self.solar_available_kw,
            fission_available_kw: self.fission_available_kw,
            generation_kw: self.generation_kw,
            requested_load_kw: self.requested_load_kw,
            served_load_kw: self.served_load_kw,
            critical_load_kw: self.critical_load_kw,
            flexible_load_kw: self.flexible_load_kw,
            battery_power_kw: self.battery_power_kw,
            battery_energy_kwh: self.battery_energy_kwh,
            battery_soc_pct,
            bus_voltage_v: self.bus_voltage_v,
            unmet_load_kw: self.unmet_load_kw,
            curtailed_generation_kw: self.curtailed_generation_kw,
            load_shed_active: self.load_shed_active,
            alarm_battery_low: battery_soc_pct < 10.0,
            alarm_bus_undervoltage: self.bus_voltage_v < 360.0,
            cumulative_generated_kwh: self.cumulative_generated_kwh,
            cumulative_requested_kwh: self.cumulative_requested_kwh,
            cumulative_served_kwh: self.cumulative_served_kwh,
            cumulative_unserved_kwh: self.cumulative_unserved_kwh,
            instantaneous_balance_error_kw: self.instantaneous_balance_error_kw,
        }
    }

    fn battery_soc_pct(&self) -> f64 {
        if self.battery_capacity_kwh <= 0.0 {
            0.0
        } else {
            self.battery_energy_kwh / self.battery_capacity_kwh * 100.0
        }
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/PowerService/ServiceInformation".to_string(),
            "ServiceSet/PowerService/Modes".to_string(),
            "ServiceSet/PowerService/StateMachine".to_string(),
            "ServiceSet/PowerService/DataAssemblies/Indicators".to_string(),
            "ServiceSet/PowerService/DataAssemblies/Parameters".to_string(),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalSimulation {
    sim_time_sec: f64,
    thermal_capacity_mj_per_c: f64,
    stored_thermal_energy_mj: f64,
    mars_ambient_temp_c: f64,
    coolant_supply_temp_c: f64,
    coolant_return_temp_c: f64,
    coolant_flow_kg_s: f64,
    radiator_deployment_pct: f64,
    equipment_heat_load_kw: f64,
    heater_power_kw: f64,
    heat_rejection_kw: f64,
    pump_electric_power_kw: f64,
    cumulative_heat_load_kwh: f64,
    cumulative_heat_rejected_kwh: f64,
    instantaneous_balance_error_kw: f64,
}

impl ThermalSimulation {
    pub fn new() -> Self {
        let thermal_capacity_mj_per_c = 25.0;
        Self {
            sim_time_sec: 0.0,
            thermal_capacity_mj_per_c,
            stored_thermal_energy_mj: thermal_capacity_mj_per_c * 22.0,
            mars_ambient_temp_c: -63.0,
            coolant_supply_temp_c: 18.0,
            coolant_return_temp_c: 24.0,
            coolant_flow_kg_s: 2.5,
            radiator_deployment_pct: 35.0,
            equipment_heat_load_kw: 24.0,
            heater_power_kw: 0.0,
            heat_rejection_kw: 24.0,
            pump_electric_power_kw: 4.0,
            cumulative_heat_load_kwh: 0.0,
            cumulative_heat_rejected_kwh: 0.0,
            instantaneous_balance_error_kw: 0.0,
        }
    }

    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        power_available: bool,
        eclss_electric_kw: f64,
        sabatier_electric_kw: f64,
        fire_heat_release_kw: f64,
        served_base_electric_kw: f64,
        mars_ambient_temp_c: f64,
    ) -> ThermalSnapshot {
        self.sim_time_sec += dt_sec;
        self.mars_ambient_temp_c = mars_ambient_temp_c.clamp(-150.0, 40.0);
        let habitat_temp_c = self.habitat_temp_c();
        self.equipment_heat_load_kw = 10.0
            + 0.85 * eclss_electric_kw.max(0.0)
            + 0.90 * sabatier_electric_kw.max(0.0)
            + fire_heat_release_kw.max(0.0)
            + 0.06 * served_base_electric_kw.max(0.0);

        let cooling_available = running && power_available;
        let target_flow = if cooling_available { 2.5 } else { 0.0 };
        self.coolant_flow_kg_s = first_order(self.coolant_flow_kg_s, target_flow, 0.35, dt_sec);
        let desired_deployment = if cooling_available {
            (20.0 + (habitat_temp_c - 20.0) * 10.0).clamp(10.0, 100.0)
        } else {
            0.0
        };
        self.radiator_deployment_pct = first_order(
            self.radiator_deployment_pct,
            desired_deployment,
            0.08,
            dt_sec,
        );
        self.heater_power_kw = if cooling_available {
            ((18.0 - habitat_temp_c) * 8.0).clamp(0.0, 25.0)
        } else {
            0.0
        };
        let active_rejection = (habitat_temp_c - self.mars_ambient_temp_c).max(0.0)
            * 0.45
            * (self.radiator_deployment_pct / 100.0)
            * (self.coolant_flow_kg_s / 2.5).clamp(0.0, 1.0);
        let passive_rejection = (habitat_temp_c - self.mars_ambient_temp_c).max(0.0) * 0.025;
        self.heat_rejection_kw = (active_rejection + passive_rejection).clamp(0.0, 65.0);
        self.pump_electric_power_kw = if cooling_available {
            1.5 + self.coolant_flow_kg_s * 0.9
        } else {
            0.0
        };

        let opening_energy_mj = self.stored_thermal_energy_mj;
        let net_heat_kw =
            self.equipment_heat_load_kw + self.heater_power_kw - self.heat_rejection_kw;
        self.stored_thermal_energy_mj += net_heat_kw * dt_sec / 1_000.0;
        let closing_temp_c = self.habitat_temp_c();
        self.coolant_return_temp_c = first_order(
            self.coolant_return_temp_c,
            closing_temp_c + 4.0 + self.equipment_heat_load_kw * 0.08,
            0.18,
            dt_sec,
        );
        self.coolant_supply_temp_c = first_order(
            self.coolant_supply_temp_c,
            if cooling_available {
                closing_temp_c - 4.0
            } else {
                closing_temp_c
            },
            0.18,
            dt_sec,
        );
        self.cumulative_heat_load_kwh +=
            (self.equipment_heat_load_kw + self.heater_power_kw) * dt_sec / 3_600.0;
        self.cumulative_heat_rejected_kwh += self.heat_rejection_kw * dt_sec / 3_600.0;
        let stored_delta_kw =
            (self.stored_thermal_energy_mj - opening_energy_mj) * 1_000.0 / dt_sec;
        self.instantaneous_balance_error_kw = self.equipment_heat_load_kw + self.heater_power_kw
            - self.heat_rejection_kw
            - stored_delta_kw;
        self.snapshot()
    }

    pub fn snapshot(&self) -> ThermalSnapshot {
        let habitat_temp_c = self.habitat_temp_c();
        let cooling_available = self.coolant_flow_kg_s > 0.5;
        ThermalSnapshot {
            timestamp_ms: now_ms(),
            sim_time_sec: self.sim_time_sec,
            mars_ambient_temp_c: self.mars_ambient_temp_c,
            habitat_temp_c,
            coolant_supply_temp_c: self.coolant_supply_temp_c,
            coolant_return_temp_c: self.coolant_return_temp_c,
            coolant_flow_kg_s: self.coolant_flow_kg_s,
            radiator_deployment_pct: self.radiator_deployment_pct,
            equipment_heat_load_kw: self.equipment_heat_load_kw,
            heater_power_kw: self.heater_power_kw,
            heat_rejection_kw: self.heat_rejection_kw,
            pump_electric_power_kw: self.pump_electric_power_kw,
            stored_thermal_energy_mj: self.stored_thermal_energy_mj,
            cumulative_heat_load_kwh: self.cumulative_heat_load_kwh,
            cumulative_heat_rejected_kwh: self.cumulative_heat_rejected_kwh,
            instantaneous_balance_error_kw: self.instantaneous_balance_error_kw,
            alarm_habitat_hot: habitat_temp_c > 30.0,
            alarm_habitat_cold: habitat_temp_c < 15.0,
            alarm_coolant_hot: self.coolant_return_temp_c > 45.0,
            cooling_available,
        }
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/ThermalService/ServiceInformation".to_string(),
            "ServiceSet/ThermalService/Modes".to_string(),
            "ServiceSet/ThermalService/StateMachine".to_string(),
            "ServiceSet/ThermalService/DataAssemblies/Indicators".to_string(),
            "ServiceSet/ThermalService/DataAssemblies/Parameters".to_string(),
            "ServiceSet/ThermalService/DataAssemblies/Alarms".to_string(),
        ]
    }

    fn habitat_temp_c(&self) -> f64 {
        self.stored_thermal_energy_mj / self.thermal_capacity_mj_per_c
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterSimulation {
    sim_time_sec: f64,
    potable_capacity_kg: f64,
    wastewater_capacity_kg: f64,
    brine_capacity_kg: f64,
    potable_water_kg: f64,
    wastewater_kg: f64,
    brine_kg: f64,
    discharged_water_kg: f64,
    crew_count: u32,
    crew_demand_kgph: f64,
    crew_water_served_kgph: f64,
    unmet_crew_water_kgph: f64,
    external_water_input_kgph: f64,
    treatment_feed_kgph: f64,
    reclaimed_water_kgph: f64,
    brine_production_kgph: f64,
    recovery_efficiency_pct: f64,
    potable_conductivity_us_cm: f64,
    potable_toc_mg_l: f64,
    microbial_cfu_ml: f64,
    treatment_power_kw: f64,
    cumulative_external_input_kg: f64,
    cumulative_crew_consumption_kg: f64,
    cumulative_reclaimed_kg: f64,
    instantaneous_balance_error_kgph: f64,
    treatment_available: bool,
    #[serde(default)]
    injected_conductivity_target_us_cm: f64,
}

impl WaterSimulation {
    pub fn new() -> Self {
        Self {
            sim_time_sec: 0.0,
            potable_capacity_kg: 3_000.0,
            wastewater_capacity_kg: 1_500.0,
            brine_capacity_kg: 600.0,
            potable_water_kg: 2_400.0,
            wastewater_kg: 400.0,
            brine_kg: 50.0,
            discharged_water_kg: 0.0,
            crew_count: 4,
            crew_demand_kgph: 0.60,
            crew_water_served_kgph: 0.60,
            unmet_crew_water_kgph: 0.0,
            external_water_input_kgph: 0.0,
            treatment_feed_kgph: 1.2,
            reclaimed_water_kgph: 1.06,
            brine_production_kgph: 0.14,
            recovery_efficiency_pct: 88.0,
            potable_conductivity_us_cm: 145.0,
            potable_toc_mg_l: 0.35,
            microbial_cfu_ml: 0.2,
            treatment_power_kw: 4.5,
            cumulative_external_input_kg: 0.0,
            cumulative_crew_consumption_kg: 0.0,
            cumulative_reclaimed_kg: 0.0,
            instantaneous_balance_error_kgph: 0.0,
            treatment_available: true,
            injected_conductivity_target_us_cm: 0.0,
        }
    }

    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        power_available: bool,
        sabatier_water_kgph: f64,
        eclss_condensate_kgph: f64,
    ) -> WaterSnapshot {
        self.sim_time_sec += dt_sec;
        let dt_hours = dt_sec / 3_600.0;
        let opening_total_kg = self.total_tracked_water_kg();
        self.external_water_input_kgph =
            sabatier_water_kgph.max(0.0) + eclss_condensate_kgph.max(0.0);
        let external_input_kg = self.external_water_input_kgph * dt_hours;
        self.wastewater_kg += external_input_kg;

        self.crew_demand_kgph = self.crew_count as f64 * 0.15;
        let available_draw_kgph = if dt_hours > 0.0 {
            self.potable_water_kg / dt_hours
        } else {
            0.0
        };
        self.crew_water_served_kgph = self.crew_demand_kgph.min(available_draw_kgph);
        self.unmet_crew_water_kgph = (self.crew_demand_kgph - self.crew_water_served_kgph).max(0.0);
        let crew_transfer_kg = self.crew_water_served_kgph * dt_hours;
        self.potable_water_kg -= crew_transfer_kg;
        self.wastewater_kg += crew_transfer_kg;

        self.treatment_available = running && power_available;
        let max_feed_kgph = if dt_hours > 0.0 {
            self.wastewater_kg / dt_hours
        } else {
            0.0
        };
        let target_feed_kgph: f64 = if self.treatment_available { 1.4 } else { 0.0 };
        self.treatment_feed_kgph = first_order(
            self.treatment_feed_kgph,
            target_feed_kgph.min(max_feed_kgph),
            0.22,
            dt_sec,
        )
        .min(max_feed_kgph);
        let treatment_feed_kg = self.treatment_feed_kgph * dt_hours;
        self.wastewater_kg -= treatment_feed_kg;
        self.recovery_efficiency_pct = if self.treatment_available {
            (90.0 - self.brine_kg / self.brine_capacity_kg * 4.0).clamp(82.0, 90.0)
        } else {
            0.0
        };
        self.reclaimed_water_kgph = self.treatment_feed_kgph * self.recovery_efficiency_pct / 100.0;
        self.brine_production_kgph = self.treatment_feed_kgph - self.reclaimed_water_kgph;
        let reclaimed_kg = self.reclaimed_water_kgph * dt_hours;
        let brine_kg = self.brine_production_kgph * dt_hours;
        self.potable_water_kg += reclaimed_kg;
        self.brine_kg += brine_kg;

        if self.potable_water_kg > self.potable_capacity_kg {
            self.discharged_water_kg += self.potable_water_kg - self.potable_capacity_kg;
            self.potable_water_kg = self.potable_capacity_kg;
        }
        if self.wastewater_kg > self.wastewater_capacity_kg {
            self.discharged_water_kg += self.wastewater_kg - self.wastewater_capacity_kg;
            self.wastewater_kg = self.wastewater_capacity_kg;
        }
        if self.brine_kg > self.brine_capacity_kg {
            self.discharged_water_kg += self.brine_kg - self.brine_capacity_kg;
            self.brine_kg = self.brine_capacity_kg;
        }

        let circulation_factor = if self.treatment_available { 1.0 } else { 0.0 };
        self.potable_conductivity_us_cm = first_order(
            self.potable_conductivity_us_cm,
            if self.injected_conductivity_target_us_cm > 0.0 {
                self.injected_conductivity_target_us_cm
            } else if self.treatment_available {
                145.0
            } else {
                620.0
            },
            0.00008 + 0.025 * circulation_factor,
            dt_sec,
        );
        self.potable_toc_mg_l = first_order(
            self.potable_toc_mg_l,
            if self.treatment_available { 0.35 } else { 4.5 },
            0.00005 + 0.018 * circulation_factor,
            dt_sec,
        );
        self.microbial_cfu_ml = first_order(
            self.microbial_cfu_ml,
            if self.treatment_available { 0.2 } else { 250.0 },
            0.00004 + 0.02 * circulation_factor,
            dt_sec,
        );
        self.treatment_power_kw = if self.treatment_available {
            2.4 + self.treatment_feed_kgph * 1.8
        } else {
            0.15
        };

        self.cumulative_external_input_kg += external_input_kg;
        self.cumulative_crew_consumption_kg += crew_transfer_kg;
        self.cumulative_reclaimed_kg += reclaimed_kg;
        let closing_total_kg = self.total_tracked_water_kg();
        self.instantaneous_balance_error_kgph = if dt_hours > 0.0 {
            (opening_total_kg + external_input_kg - closing_total_kg) / dt_hours
        } else {
            0.0
        };
        self.snapshot()
    }

    pub fn snapshot(&self) -> WaterSnapshot {
        WaterSnapshot {
            timestamp_ms: now_ms(),
            sim_time_sec: self.sim_time_sec,
            potable_water_kg: self.potable_water_kg,
            potable_capacity_kg: self.potable_capacity_kg,
            wastewater_kg: self.wastewater_kg,
            wastewater_capacity_kg: self.wastewater_capacity_kg,
            brine_kg: self.brine_kg,
            brine_capacity_kg: self.brine_capacity_kg,
            discharged_water_kg: self.discharged_water_kg,
            crew_demand_kgph: self.crew_demand_kgph,
            crew_water_served_kgph: self.crew_water_served_kgph,
            unmet_crew_water_kgph: self.unmet_crew_water_kgph,
            external_water_input_kgph: self.external_water_input_kgph,
            treatment_feed_kgph: self.treatment_feed_kgph,
            reclaimed_water_kgph: self.reclaimed_water_kgph,
            brine_production_kgph: self.brine_production_kgph,
            recovery_efficiency_pct: self.recovery_efficiency_pct,
            potable_conductivity_us_cm: self.potable_conductivity_us_cm,
            potable_toc_mg_l: self.potable_toc_mg_l,
            microbial_cfu_ml: self.microbial_cfu_ml,
            treatment_power_kw: self.treatment_power_kw,
            cumulative_external_input_kg: self.cumulative_external_input_kg,
            cumulative_crew_consumption_kg: self.cumulative_crew_consumption_kg,
            cumulative_reclaimed_kg: self.cumulative_reclaimed_kg,
            instantaneous_balance_error_kgph: self.instantaneous_balance_error_kgph,
            treatment_available: self.treatment_available,
            alarm_potable_low: self.potable_water_kg < self.potable_capacity_kg * 0.20,
            alarm_wastewater_high: self.wastewater_kg > self.wastewater_capacity_kg * 0.85,
            alarm_brine_high: self.brine_kg > self.brine_capacity_kg * 0.85,
            alarm_water_quality: self.potable_conductivity_us_cm > 500.0
                || self.potable_toc_mg_l > 2.0
                || self.microbial_cfu_ml > 100.0,
        }
    }

    pub fn set_conductivity_contamination_target(
        &mut self,
        target_us_cm: f64,
    ) -> Result<(), String> {
        if !target_us_cm.is_finite() || !(0.0..=2_000.0).contains(&target_us_cm) {
            return Err(
                "conductivity contamination target must be finite and within 0..=2000".to_string(),
            );
        }
        self.injected_conductivity_target_us_cm = target_us_cm;
        Ok(())
    }

    pub fn conductivity_contamination_target(&self) -> f64 {
        self.injected_conductivity_target_us_cm
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/WaterService/ServiceInformation".to_string(),
            "ServiceSet/WaterService/Modes".to_string(),
            "ServiceSet/WaterService/StateMachine".to_string(),
            "ServiceSet/WaterService/DataAssemblies/Inventories".to_string(),
            "ServiceSet/WaterService/DataAssemblies/Quality".to_string(),
            "ServiceSet/WaterService/DataAssemblies/Alarms".to_string(),
        ]
    }

    fn total_tracked_water_kg(&self) -> f64 {
        self.potable_water_kg + self.wastewater_kg + self.brine_kg + self.discharged_water_kg
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySimulation {
    sim_time_sec: f64,
    habitat_air_mass_kg: f64,
    habitat_volume_m3: f64,
    habitat_temp_c: f64,
    exterior_pressure_kpa: f64,
    nominal_leak_kg_s: f64,
    injected_leak_kg_s: f64,
    cumulative_leaked_air_kg: f64,
    cumulative_makeup_air_kg: f64,
    instantaneous_mass_balance_error_kgph: f64,
    previous_pressure_kpa: f64,
    pressure_decay_kpa_per_min: f64,
    smoke_ppm: f64,
    carbon_monoxide_ppm: f64,
    fire_source_kw: f64,
    suppression_agent_kg: f64,
    structural_integrity_pct: f64,
    cumulative_internal_dose_msv: f64,
    habitat_isolated: bool,
    monitoring_available: bool,
    makeup_gas_kgph: f64,
    suppression_flow_kgph: f64,
    external_radiation_msv_h: f64,
    internal_radiation_msv_h: f64,
    safety_power_kw: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyHazardSettings {
    pub injected_leak_kg_s: f64,
    pub fire_source_kw: f64,
    pub habitat_isolated: bool,
}

impl SafetySimulation {
    pub fn new() -> Self {
        let habitat_volume_m3 = 600.0;
        let habitat_temp_c = 22.0;
        let initial_pressure_kpa = 101.3;
        let habitat_air_mass_kg = initial_pressure_kpa * 1_000.0 * habitat_volume_m3
            / (287.05 * (habitat_temp_c + 273.15));
        Self {
            sim_time_sec: 0.0,
            habitat_air_mass_kg,
            habitat_volume_m3,
            habitat_temp_c,
            exterior_pressure_kpa: 0.61,
            nominal_leak_kg_s: 2.0e-6,
            injected_leak_kg_s: 0.0,
            cumulative_leaked_air_kg: 0.0,
            cumulative_makeup_air_kg: 0.0,
            instantaneous_mass_balance_error_kgph: 0.0,
            previous_pressure_kpa: initial_pressure_kpa,
            pressure_decay_kpa_per_min: 0.0,
            smoke_ppm: 0.0,
            carbon_monoxide_ppm: 0.4,
            fire_source_kw: 0.0,
            suppression_agent_kg: 80.0,
            structural_integrity_pct: 100.0,
            cumulative_internal_dose_msv: 0.0,
            habitat_isolated: false,
            monitoring_available: true,
            makeup_gas_kgph: 0.0,
            suppression_flow_kgph: 0.0,
            external_radiation_msv_h: 0.025,
            internal_radiation_msv_h: 0.003,
            safety_power_kw: 2.4,
        }
    }

    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        critical_power_available: bool,
        eclss_running: bool,
        exterior_pressure_kpa: f64,
        external_radiation_msv_h: f64,
    ) -> SafetySnapshot {
        if dt_sec <= 0.0 {
            return self.snapshot();
        }
        self.sim_time_sec += dt_sec;
        self.exterior_pressure_kpa = exterior_pressure_kpa.clamp(0.0, 2.0);
        self.external_radiation_msv_h = external_radiation_msv_h.max(0.0);
        self.monitoring_available = running && critical_power_available;
        let dt_hours = dt_sec / 3_600.0;
        let opening_ledger_kg = self.habitat_air_mass_kg + self.cumulative_leaked_air_kg;

        let total_leak_kg_s = self.nominal_leak_kg_s + self.injected_leak_kg_s;
        let leak_kg = (total_leak_kg_s * dt_sec).min(self.habitat_air_mass_kg);
        self.habitat_air_mass_kg -= leak_kg;
        self.cumulative_leaked_air_kg += leak_kg;

        let pressure_before_makeup = self.pressure_kpa();
        let pressure_deficit_kpa = (101.3 - pressure_before_makeup).max(0.0);
        self.makeup_gas_kgph = if eclss_running && critical_power_available {
            (total_leak_kg_s * 3_600.0 + pressure_deficit_kpa * 0.25).clamp(0.0, 3.0)
        } else {
            0.0
        };
        let makeup_kg = self.makeup_gas_kgph * dt_hours;
        self.habitat_air_mass_kg += makeup_kg;
        self.cumulative_makeup_air_kg += makeup_kg;

        let pressure_kpa = self.pressure_kpa();
        self.pressure_decay_kpa_per_min =
            (self.previous_pressure_kpa - pressure_kpa) / dt_sec * 60.0;
        self.previous_pressure_kpa = pressure_kpa;

        let suppression_enabled = self.monitoring_available
            && self.fire_source_kw >= 5.0
            && self.suppression_agent_kg > 0.0;
        self.suppression_flow_kgph = if suppression_enabled {
            (self.fire_source_kw * 0.16).clamp(0.5, 12.0)
        } else {
            0.0
        };
        let suppression_used_kg =
            (self.suppression_flow_kgph * dt_hours).min(self.suppression_agent_kg);
        self.suppression_agent_kg -= suppression_used_kg;
        if suppression_enabled {
            self.fire_source_kw = (self.fire_source_kw - suppression_used_kg * 30.0).max(0.0);
        }

        let ventilation_factor = if running && critical_power_available {
            0.22
        } else {
            0.001
        };
        self.smoke_ppm += self.fire_source_kw * 0.018 * dt_sec;
        self.carbon_monoxide_ppm += self.fire_source_kw * 0.004 * dt_sec;
        self.smoke_ppm = first_order(self.smoke_ppm, 0.0, ventilation_factor, dt_sec).max(0.0);
        self.carbon_monoxide_ppm =
            first_order(self.carbon_monoxide_ppm, 0.4, ventilation_factor, dt_sec).max(0.0);

        let pressure_differential_kpa = (pressure_kpa - self.exterior_pressure_kpa).max(0.0);
        let strain_microstrain = pressure_differential_kpa / 140.0 * 1_200.0
            / (self.structural_integrity_pct / 100.0).max(0.1);
        if strain_microstrain > 1_150.0 {
            self.structural_integrity_pct = (self.structural_integrity_pct
                - (strain_microstrain - 1_150.0) * 1.0e-7 * dt_sec)
                .max(0.0);
        }

        let shielding_factor = if self.habitat_isolated { 0.09 } else { 0.12 };
        self.internal_radiation_msv_h = self.external_radiation_msv_h * shielding_factor;
        self.cumulative_internal_dose_msv += self.internal_radiation_msv_h * dt_hours;
        self.safety_power_kw = if running && critical_power_available {
            2.4 + if suppression_enabled { 1.6 } else { 0.0 }
        } else {
            0.15
        };

        let closing_ledger_kg = self.habitat_air_mass_kg + self.cumulative_leaked_air_kg;
        self.instantaneous_mass_balance_error_kgph =
            (opening_ledger_kg + makeup_kg - closing_ledger_kg) / dt_hours;
        self.snapshot()
    }

    pub fn set_hazards(
        &mut self,
        injected_leak_kg_s: Option<f64>,
        fire_source_kw: Option<f64>,
        habitat_isolated: Option<bool>,
    ) -> Result<(), String> {
        if let Some(value) = injected_leak_kg_s {
            if !value.is_finite() || !(0.0..=0.25).contains(&value) {
                return Err("injected_leak_kg_s must be finite and within 0..=0.25".to_string());
            }
            self.injected_leak_kg_s = value;
        }
        if let Some(value) = fire_source_kw {
            if !value.is_finite() || !(0.0..=2_000.0).contains(&value) {
                return Err("fire_source_kw must be finite and within 0..=2000".to_string());
            }
            self.fire_source_kw = value;
        }
        if let Some(value) = habitat_isolated {
            self.habitat_isolated = value;
        }
        Ok(())
    }

    pub fn hazard_settings(&self) -> SafetyHazardSettings {
        SafetyHazardSettings {
            injected_leak_kg_s: self.injected_leak_kg_s,
            fire_source_kw: self.fire_source_kw,
            habitat_isolated: self.habitat_isolated,
        }
    }

    pub fn snapshot(&self) -> SafetySnapshot {
        let pressure_kpa = self.pressure_kpa();
        let pressure_differential_kpa = (pressure_kpa - self.exterior_pressure_kpa).max(0.0);
        let strain_microstrain = pressure_differential_kpa / 140.0 * 1_200.0
            / (self.structural_integrity_pct / 100.0).max(0.1);
        SafetySnapshot {
            timestamp_ms: now_ms(),
            sim_time_sec: self.sim_time_sec,
            habitat_air_mass_kg: self.habitat_air_mass_kg,
            habitat_volume_m3: self.habitat_volume_m3,
            habitat_pressure_kpa: pressure_kpa,
            exterior_pressure_kpa: self.exterior_pressure_kpa,
            pressure_decay_kpa_per_min: self.pressure_decay_kpa_per_min,
            leak_rate_kgph: (self.nominal_leak_kg_s + self.injected_leak_kg_s) * 3_600.0,
            makeup_gas_kgph: self.makeup_gas_kgph,
            cumulative_leaked_air_kg: self.cumulative_leaked_air_kg,
            cumulative_makeup_air_kg: self.cumulative_makeup_air_kg,
            instantaneous_mass_balance_error_kgph: self.instantaneous_mass_balance_error_kgph,
            smoke_ppm: self.smoke_ppm,
            carbon_monoxide_ppm: self.carbon_monoxide_ppm,
            fire_heat_release_kw: self.fire_source_kw,
            suppression_agent_kg: self.suppression_agent_kg,
            suppression_flow_kgph: self.suppression_flow_kgph,
            habitat_isolated: self.habitat_isolated,
            pressure_shell_strain_microstrain: strain_microstrain,
            structural_integrity_pct: self.structural_integrity_pct,
            external_radiation_msv_h: self.external_radiation_msv_h,
            internal_radiation_msv_h: self.internal_radiation_msv_h,
            cumulative_internal_dose_msv: self.cumulative_internal_dose_msv,
            safety_power_kw: self.safety_power_kw,
            monitoring_available: self.monitoring_available,
            alarm_low_pressure: pressure_kpa < 95.0,
            alarm_rapid_decompression: self.pressure_decay_kpa_per_min > 0.5,
            alarm_fire: self.fire_source_kw >= 5.0 || self.smoke_ppm >= 8.0,
            alarm_toxic_gas: self.carbon_monoxide_ppm >= 25.0,
            alarm_radiation: self.internal_radiation_msv_h >= 0.05,
            alarm_structural: strain_microstrain >= 1_100.0 || self.structural_integrity_pct < 90.0,
        }
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/SafetyService/ServiceInformation".to_string(),
            "ServiceSet/SafetyService/Modes".to_string(),
            "ServiceSet/SafetyService/StateMachine".to_string(),
            "ServiceSet/SafetyService/DataAssemblies/PressureIntegrity".to_string(),
            "ServiceSet/SafetyService/DataAssemblies/FireGas".to_string(),
            "ServiceSet/SafetyService/DataAssemblies/Radiation".to_string(),
            "ServiceSet/SafetyService/DataAssemblies/Alarms".to_string(),
        ]
    }

    fn pressure_kpa(&self) -> f64 {
        self.habitat_air_mass_kg * 287.05 * (self.habitat_temp_c + 273.15)
            / self.habitat_volume_m3
            / 1_000.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclssSimulation {
    #[serde(default)]
    sim_time_sec: f64,
    cabin_pressure_kpa: f64,
    o2_percent: f64,
    co2_ppm: f64,
    humidity_pct: f64,
    water_recovery_pct: f64,
    co2_capture_kgph: f64,
    o2_generation_kgph: f64,
    power_kw: f64,
    #[serde(default)]
    reliability: EclssReliabilityState,
    #[serde(default = "default_true")]
    co2_scrubber_running: bool,
    #[serde(default = "default_true")]
    electrolyzer_running: bool,
    #[serde(default = "default_true")]
    water_processor_running: bool,
    #[serde(default)]
    branch_isolated: bool,
    #[serde(default)]
    safe_haven_enabled: bool,
    #[serde(default)]
    co2_removal_flow_target_kgph: Option<f64>,
    #[serde(default)]
    o2_generation_target_kgph: Option<f64>,
    #[serde(default)]
    water_recovery_target_pct: Option<f64>,
    #[serde(default)]
    humidity_target_pct: Option<f64>,
    #[serde(default)]
    cabin_pressure_target_kpa: Option<f64>,
}

impl EclssSimulation {
    pub fn new() -> Self {
        Self {
            sim_time_sec: 0.0,
            cabin_pressure_kpa: 101.3,
            o2_percent: 21.0,
            co2_ppm: 950.0,
            humidity_pct: 44.0,
            water_recovery_pct: 83.0,
            co2_capture_kgph: 0.7,
            o2_generation_kgph: 0.8,
            power_kw: 11.0,
            reliability: EclssReliabilityState::default(),
            co2_scrubber_running: true,
            electrolyzer_running: true,
            water_processor_running: true,
            branch_isolated: false,
            safe_haven_enabled: false,
            co2_removal_flow_target_kgph: None,
            o2_generation_target_kgph: None,
            water_recovery_target_pct: None,
            humidity_target_pct: None,
            cabin_pressure_target_kpa: None,
        }
    }

    pub fn set_co2_scrubber_running(&mut self, running: bool) {
        self.co2_scrubber_running = running;
    }

    pub fn set_electrolyzer_running(&mut self, running: bool) {
        self.electrolyzer_running = running;
    }

    pub fn set_water_processor_running(&mut self, running: bool) {
        self.water_processor_running = running;
    }

    pub fn set_branch_isolated(&mut self, isolated: bool) {
        self.branch_isolated = isolated;
    }

    pub fn set_safe_haven_enabled(&mut self, enabled: bool) {
        self.safe_haven_enabled = enabled;
    }

    pub fn set_co2_removal_flow_target(&mut self, target_kgph: f64) {
        self.co2_removal_flow_target_kgph = Some(target_kgph.clamp(0.0, 3.0));
    }

    pub fn set_o2_generation_target(&mut self, target_kgph: f64) {
        self.o2_generation_target_kgph = Some(target_kgph.clamp(0.0, 3.0));
    }

    pub fn set_water_recovery_target(&mut self, target_pct: f64) {
        self.water_recovery_target_pct = Some(target_pct.clamp(0.0, 100.0));
    }

    pub fn set_humidity_target(&mut self, target_pct: f64) {
        self.humidity_target_pct = Some(target_pct.clamp(20.0, 80.0));
    }

    pub fn set_cabin_pressure_target(&mut self, target_kpa: f64) {
        self.cabin_pressure_target_kpa = Some(target_kpa.clamp(20.0, 110.0));
    }

    pub fn co2_scrubber_running(&self) -> bool {
        self.co2_scrubber_running
    }

    pub fn electrolyzer_running(&self) -> bool {
        self.electrolyzer_running
    }

    pub fn water_processor_running(&self) -> bool {
        self.water_processor_running
    }

    pub fn branch_isolated(&self) -> bool {
        self.branch_isolated
    }

    pub fn safe_haven_enabled(&self) -> bool {
        self.safe_haven_enabled
    }

    pub fn step(&mut self, dt_sec: f64, running: bool) -> EclssSnapshot {
        if dt_sec > 0.0 {
            self.sim_time_sec += dt_sec;
        }
        let capacity = self.reliability.step(dt_sec, running);

        // Safe haven forces every life-support process online regardless of
        // individual toggles; branch isolation disconnects the cabin loop
        // from any process that hasn't been overridden by safe haven.
        let scrubber_active = running
            && (self.safe_haven_enabled || (self.co2_scrubber_running && !self.branch_isolated));
        let electrolyzer_active = running
            && (self.safe_haven_enabled || (self.electrolyzer_running && !self.branch_isolated));
        let water_processor_active = running
            && (self.safe_haven_enabled || (self.water_processor_running && !self.branch_isolated));

        if scrubber_active {
            let co2_target_capture = self.co2_removal_flow_target_kgph.unwrap_or_else(|| {
                ((self.co2_ppm - 700.0) / 500.0).clamp(0.25, 1.8) * capacity.carbon_dioxide_removal
            });
            self.co2_capture_kgph =
                first_order(self.co2_capture_kgph, co2_target_capture, 0.35, dt_sec);
            let crew_co2_load = 65.0;
            self.co2_ppm += dt_sec * (crew_co2_load - self.co2_capture_kgph * 80.0);
            self.co2_ppm = self.co2_ppm.clamp(400.0, 9000.0);
        } else {
            self.co2_capture_kgph = first_order(self.co2_capture_kgph, 0.0, 0.5, dt_sec);
            self.co2_ppm += dt_sec * 38.0;
            self.co2_ppm = self.co2_ppm.clamp(400.0, 9000.0);
        }

        if electrolyzer_active {
            let o2_target_gen = self.o2_generation_target_kgph.unwrap_or_else(|| {
                ((21.1 - self.o2_percent) * 0.9 + 0.7).clamp(0.5, 1.6) * capacity.oxygen_generation
            });
            self.o2_generation_kgph =
                first_order(self.o2_generation_kgph, o2_target_gen, 0.25, dt_sec);
            self.o2_percent += dt_sec * (self.o2_generation_kgph * 0.015 - 0.004);
            self.o2_percent = self.o2_percent.clamp(18.0, 24.0);
        } else {
            self.o2_generation_kgph = first_order(self.o2_generation_kgph, 0.0, 0.5, dt_sec);
            self.o2_percent = first_order(self.o2_percent, 20.4, 0.02, dt_sec);
        }

        if water_processor_active {
            let humidity_target = self
                .humidity_target_pct
                .unwrap_or(47.0 + (1.0 - capacity.humidity_control) * 18.0);
            self.humidity_pct = first_order(
                self.humidity_pct,
                humidity_target,
                0.12 * capacity.humidity_control.max(0.02),
                dt_sec,
            );
            let recovery_target = self
                .water_recovery_target_pct
                .unwrap_or(20.0 + 68.0 * capacity.water_recovery);
            self.water_recovery_pct =
                first_order(self.water_recovery_pct, recovery_target, 0.08, dt_sec);
        } else {
            self.humidity_pct = first_order(self.humidity_pct, 54.0, 0.04, dt_sec);
            self.water_recovery_pct = first_order(self.water_recovery_pct, 64.0, 0.04, dt_sec);
        }

        let cabin_pressure_target = self.cabin_pressure_target_kpa.unwrap_or(101.3);
        self.cabin_pressure_kpa =
            first_order(self.cabin_pressure_kpa, cabin_pressure_target, 0.05, dt_sec);

        self.power_kw = first_order(
            self.power_kw,
            if running { 8.5 } else { 2.0 }
                + self.co2_capture_kgph * 2.8
                + self.o2_generation_kgph * 2.4,
            if running { 0.3 } else { 0.4 },
            dt_sec,
        );

        self.snapshot()
    }

    pub fn snapshot(&self) -> EclssSnapshot {
        let reliability = self.reliability.snapshot();
        EclssSnapshot {
            timestamp_ms: now_ms(),
            sim_time_sec: self.sim_time_sec,
            cabin_pressure_kpa: self.cabin_pressure_kpa,
            o2_percent: self.o2_percent,
            co2_ppm: self.co2_ppm,
            humidity_pct: self.humidity_pct,
            water_recovery_pct: self.water_recovery_pct,
            co2_capture_kgph: self.co2_capture_kgph,
            o2_generation_kgph: self.o2_generation_kgph,
            power_kw: self.power_kw,
            alarm_high_co2: self.co2_ppm > 2500.0,
            alarm_low_o2: self.o2_percent < 19.3,
            alarm_maintenance_required: reliability.maintenance_backlog > 0,
            reliability,
            co2_scrubber_running: self.co2_scrubber_running,
            electrolyzer_running: self.electrolyzer_running,
            water_processor_running: self.water_processor_running,
            branch_isolated: self.branch_isolated,
            safe_haven_enabled: self.safe_haven_enabled,
        }
    }

    pub fn inject_component_failure(&mut self, component_id: &str) -> Result<(), String> {
        self.reliability.inject_failure(component_id)
    }

    pub fn inject_component_degradation(
        &mut self,
        component_id: &str,
        health_pct: f64,
    ) -> Result<(), String> {
        self.reliability
            .inject_degradation(component_id, health_pct)
    }

    pub fn start_component_repair(
        &mut self,
        component_id: &str,
    ) -> Result<EclssRepairRequest, String> {
        self.reliability.start_repair(component_id)
    }

    pub fn apply_component_repair_labor(
        &mut self,
        work_order_id: &str,
        component_id: &str,
        labor_hours: f64,
    ) -> Result<(), String> {
        self.reliability
            .apply_repair_labor(work_order_id, component_id, labor_hours)
    }

    pub fn cancel_component_repair(
        &mut self,
        work_order_id: &str,
        component_id: &str,
    ) -> Result<(), String> {
        self.reliability
            .cancel_pending_repair(work_order_id, component_id)
    }

    pub fn add_component_spares(
        &mut self,
        component_id: &str,
        quantity: u32,
    ) -> Result<(), String> {
        self.reliability.add_spares(component_id, quantity)
    }

    pub fn drain_reliability_events(&mut self) -> Vec<ReliabilityEvent> {
        self.reliability.drain_events()
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/EclssService/ServiceInformation".to_string(),
            "ServiceSet/EclssService/Modes".to_string(),
            "ServiceSet/EclssService/StateMachine".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Indicators".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Parameters".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Reliability".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Maintenance".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Control/OperatorCommands/Req".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Control/OperatorCommands/Rsp".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Control/RemoteCommands/Req".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Control/RemoteCommands/Rsp".to_string(),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabatierSimulation {
    reactor_temp_c: f64,
    reactor_pressure_bar: f64,
    co2_feed_kgph: f64,
    h2_feed_kgph: f64,
    conversion_efficiency_pct: f64,
    methane_production_kgph: f64,
    water_production_kgph: f64,
    catalyst_health_pct: f64,
    power_kw: f64,
    #[serde(default = "default_true")]
    methanation_enabled: bool,
    #[serde(default = "default_true")]
    feed_conditioning_enabled: bool,
    #[serde(default)]
    hydrogen_recovery_enabled: bool,
    #[serde(default)]
    catalyst_regen_active: bool,
    #[serde(default)]
    emergency_vent_active: bool,
    #[serde(default = "default_hundred")]
    co2_feed_valve_pct: f64,
    #[serde(default = "default_hundred")]
    h2_feed_valve_pct: f64,
    #[serde(default)]
    reactor_temp_target_c: Option<f64>,
    #[serde(default)]
    reactor_pressure_target_bar: Option<f64>,
}

impl SabatierSimulation {
    pub fn new() -> Self {
        Self {
            reactor_temp_c: 215.0,
            reactor_pressure_bar: 3.5,
            co2_feed_kgph: 0.0,
            h2_feed_kgph: 0.0,
            conversion_efficiency_pct: 0.0,
            methane_production_kgph: 0.0,
            water_production_kgph: 0.0,
            catalyst_health_pct: 99.5,
            power_kw: 0.9,
            methanation_enabled: true,
            feed_conditioning_enabled: true,
            hydrogen_recovery_enabled: false,
            catalyst_regen_active: false,
            emergency_vent_active: false,
            co2_feed_valve_pct: 100.0,
            h2_feed_valve_pct: 100.0,
            reactor_temp_target_c: None,
            reactor_pressure_target_bar: None,
        }
    }

    pub fn set_methanation_enabled(&mut self, enabled: bool) {
        self.methanation_enabled = enabled;
    }

    pub fn set_feed_conditioning_enabled(&mut self, enabled: bool) {
        self.feed_conditioning_enabled = enabled;
    }

    pub fn set_hydrogen_recovery_enabled(&mut self, enabled: bool) {
        self.hydrogen_recovery_enabled = enabled;
    }

    pub fn set_catalyst_regen_active(&mut self, active: bool) {
        self.catalyst_regen_active = active;
    }

    pub fn set_emergency_vent_active(&mut self, active: bool) {
        self.emergency_vent_active = active;
    }

    pub fn set_co2_feed_valve_pct(&mut self, pct: f64) {
        self.co2_feed_valve_pct = pct.clamp(0.0, 100.0);
    }

    pub fn set_h2_feed_valve_pct(&mut self, pct: f64) {
        self.h2_feed_valve_pct = pct.clamp(0.0, 100.0);
    }

    pub fn set_reactor_temp_target(&mut self, target_c: f64) {
        self.reactor_temp_target_c = Some(target_c.clamp(150.0, 390.0));
    }

    pub fn set_reactor_pressure_target(&mut self, target_bar: f64) {
        self.reactor_pressure_target_bar = Some(target_bar.clamp(1.0, 15.0));
    }

    /// Coordinated safe shutdown: stands down methanation, feed conditioning,
    /// and hydrogen recovery together.
    pub fn safe_shutdown(&mut self) {
        self.methanation_enabled = false;
        self.feed_conditioning_enabled = false;
        self.hydrogen_recovery_enabled = false;
    }

    pub fn methanation_enabled(&self) -> bool {
        self.methanation_enabled
    }

    pub fn feed_conditioning_enabled(&self) -> bool {
        self.feed_conditioning_enabled
    }

    pub fn hydrogen_recovery_enabled(&self) -> bool {
        self.hydrogen_recovery_enabled
    }

    pub fn catalyst_regen_active(&self) -> bool {
        self.catalyst_regen_active
    }

    pub fn emergency_vent_active(&self) -> bool {
        self.emergency_vent_active
    }

    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        eclss_co2_available_kgph: f64,
    ) -> SabatierSnapshot {
        let standby_temp_c = 215.0;
        let standby_pressure_bar = 3.5;

        if self.emergency_vent_active {
            // Rapid, forced vent to standby conditions; production stops immediately.
            self.reactor_temp_c = first_order(self.reactor_temp_c, standby_temp_c, 0.6, dt_sec);
            self.reactor_pressure_bar =
                first_order(self.reactor_pressure_bar, standby_pressure_bar, 0.6, dt_sec);
            self.co2_feed_kgph = 0.0;
            self.h2_feed_kgph = 0.0;
            self.conversion_efficiency_pct = 0.0;
            self.methane_production_kgph = 0.0;
            self.water_production_kgph = 0.0;
            self.power_kw = first_order(self.power_kw, 0.9, 0.4, dt_sec);

            // Vent auto-clears once the reactor has actually reached standby conditions.
            if (self.reactor_temp_c - standby_temp_c).abs() < 1.0
                && (self.reactor_pressure_bar - standby_pressure_bar).abs() < 0.1
            {
                self.emergency_vent_active = false;
            }
        } else if self.catalyst_regen_active {
            // Offline for catalyst regeneration: production stops, health recovers.
            self.reactor_temp_c = first_order(self.reactor_temp_c, standby_temp_c, 0.22, dt_sec);
            self.reactor_pressure_bar = first_order(
                self.reactor_pressure_bar,
                standby_pressure_bar,
                0.22,
                dt_sec,
            );
            self.co2_feed_kgph = first_order(self.co2_feed_kgph, 0.0, 0.45, dt_sec);
            self.h2_feed_kgph = first_order(self.h2_feed_kgph, 0.0, 0.45, dt_sec);
            self.conversion_efficiency_pct =
                first_order(self.conversion_efficiency_pct, 0.0, 0.5, dt_sec);
            self.methane_production_kgph =
                first_order(self.methane_production_kgph, 0.0, 0.5, dt_sec);
            self.water_production_kgph = first_order(self.water_production_kgph, 0.0, 0.5, dt_sec);
            self.power_kw = first_order(self.power_kw, 1.2, 0.3, dt_sec);
            self.catalyst_health_pct += dt_sec * 0.05;
            self.catalyst_health_pct = self.catalyst_health_pct.clamp(70.0, 100.0);
        } else if running && self.methanation_enabled && self.feed_conditioning_enabled {
            let temp_target = self.reactor_temp_target_c.unwrap_or(325.0);
            let pressure_target = self.reactor_pressure_target_bar.unwrap_or(9.0);
            self.reactor_temp_c = first_order(self.reactor_temp_c, temp_target, 0.18, dt_sec);
            self.reactor_pressure_bar =
                first_order(self.reactor_pressure_bar, pressure_target, 0.18, dt_sec);

            let co2_target =
                eclss_co2_available_kgph.clamp(0.2, 1.6) * (self.co2_feed_valve_pct / 100.0);
            self.co2_feed_kgph = first_order(self.co2_feed_kgph, co2_target, 0.3, dt_sec);
            let h2_target = self.co2_feed_kgph * 0.182 * (self.h2_feed_valve_pct / 100.0);
            self.h2_feed_kgph = first_order(self.h2_feed_kgph, h2_target, 0.25, dt_sec);

            let temperature_factor =
                1.0 - ((self.reactor_temp_c - 340.0).abs() / 220.0).clamp(0.0, 0.35);
            let catalyst_factor = (self.catalyst_health_pct / 100.0).clamp(0.65, 1.0);
            let recovery_bonus = if self.hydrogen_recovery_enabled {
                1.05
            } else {
                1.0
            };
            self.conversion_efficiency_pct =
                (84.0 * temperature_factor * catalyst_factor * recovery_bonus).clamp(60.0, 91.0);

            let efficiency = self.conversion_efficiency_pct / 100.0;
            self.methane_production_kgph = self.co2_feed_kgph * 0.364 * efficiency;
            self.water_production_kgph = self.co2_feed_kgph * 0.818 * efficiency;
            self.power_kw =
                first_order(self.power_kw, 3.4 + self.co2_feed_kgph * 1.9, 0.22, dt_sec);

            self.catalyst_health_pct -= dt_sec * 0.00065;
            self.catalyst_health_pct = self.catalyst_health_pct.clamp(70.0, 100.0);
        } else {
            self.reactor_temp_c = first_order(self.reactor_temp_c, standby_temp_c, 0.22, dt_sec);
            self.reactor_pressure_bar = first_order(
                self.reactor_pressure_bar,
                standby_pressure_bar,
                0.22,
                dt_sec,
            );
            self.co2_feed_kgph = first_order(self.co2_feed_kgph, 0.0, 0.45, dt_sec);
            self.h2_feed_kgph = first_order(self.h2_feed_kgph, 0.0, 0.45, dt_sec);
            self.conversion_efficiency_pct =
                first_order(self.conversion_efficiency_pct, 0.0, 0.5, dt_sec);
            self.methane_production_kgph =
                first_order(self.methane_production_kgph, 0.0, 0.5, dt_sec);
            self.water_production_kgph = first_order(self.water_production_kgph, 0.0, 0.5, dt_sec);
            self.power_kw = first_order(self.power_kw, 0.9, 0.3, dt_sec);
        }

        self.snapshot()
    }

    pub fn snapshot(&self) -> SabatierSnapshot {
        SabatierSnapshot {
            timestamp_ms: now_ms(),
            reactor_temp_c: self.reactor_temp_c,
            reactor_pressure_bar: self.reactor_pressure_bar,
            co2_feed_kgph: self.co2_feed_kgph,
            h2_feed_kgph: self.h2_feed_kgph,
            conversion_efficiency_pct: self.conversion_efficiency_pct,
            methane_production_kgph: self.methane_production_kgph,
            water_production_kgph: self.water_production_kgph,
            catalyst_health_pct: self.catalyst_health_pct,
            power_kw: self.power_kw,
            alarm_reactor_temp: self.reactor_temp_c > 390.0,
            methanation_enabled: self.methanation_enabled,
            feed_conditioning_enabled: self.feed_conditioning_enabled,
            hydrogen_recovery_enabled: self.hydrogen_recovery_enabled,
            catalyst_regen_active: self.catalyst_regen_active,
            emergency_vent_active: self.emergency_vent_active,
        }
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/SabatierService/ServiceInformation".to_string(),
            "ServiceSet/SabatierService/Modes".to_string(),
            "ServiceSet/SabatierService/StateMachine".to_string(),
            "ServiceSet/SabatierService/DataAssemblies/Indicators".to_string(),
            "ServiceSet/SabatierService/DataAssemblies/Parameters".to_string(),
            "ServiceSet/SabatierService/DataAssemblies/Control/OperatorCommands/Req".to_string(),
            "ServiceSet/SabatierService/DataAssemblies/Control/OperatorCommands/Rsp".to_string(),
            "ServiceSet/SabatierService/DataAssemblies/Control/RemoteCommands/Req".to_string(),
            "ServiceSet/SabatierService/DataAssemblies/Control/RemoteCommands/Rsp".to_string(),
        ]
    }
}

fn first_order(current: f64, target: f64, rate: f64, dt_sec: f64) -> f64 {
    current + (target - current) * (1.0 - (-rate * dt_sec).exp())
}

fn default_true() -> bool {
    true
}

fn default_hundred() -> f64 {
    100.0
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_balance_closes_each_step() {
        let mut power = PowerSimulation::new();
        for _ in 0..10_000 {
            let snapshot = power.step(0.05, true, 450.0, 0.2, 12.0, 4.0, 4.5, 2.4, 1.5, 5.0, 2.0);
            assert!(snapshot.instantaneous_balance_error_kw.abs() < 1.0e-9);
            assert!(snapshot.battery_energy_kwh >= 0.0);
            assert!(snapshot.battery_energy_kwh <= 500.0);
        }
    }

    #[test]
    fn shared_environment_inputs_are_authoritative_across_consumers() {
        let mut power = PowerSimulation::new();
        let power_snapshot = power.step(1.0, true, 321.5, 1.25, 12.0, 4.0, 4.5, 2.4, 1.5, 5.0, 2.0);
        assert_eq!(power_snapshot.solar_irradiance_w_m2, 321.5);
        assert_eq!(power_snapshot.dust_opacity, 1.25);

        let mut thermal = ThermalSimulation::new();
        let thermal_snapshot = thermal.step(1.0, true, true, 12.0, 5.0, 0.0, 45.0, -91.25);
        assert_eq!(thermal_snapshot.mars_ambient_temp_c, -91.25);

        let mut safety = SafetySimulation::new();
        let safety_snapshot = safety.step(1.0, true, true, true, 0.734, 0.42);
        assert_eq!(safety_snapshot.exterior_pressure_kpa, 0.734);
        assert_eq!(safety_snapshot.external_radiation_msv_h, 0.42);
        assert!(safety_snapshot.alarm_radiation);
    }

    #[test]
    fn battery_discharges_during_martian_night() {
        let mut power = PowerSimulation::new();
        let opening_energy = power.battery_energy_kwh;
        let snapshot = power.step(60.0, true, 0.0, 0.2, 12.0, 4.0, 4.5, 2.4, 1.5, 5.0, 2.0);
        assert_eq!(snapshot.solar_available_kw, 0.0);
        assert!(snapshot.battery_power_kw < 0.0);
        assert!(snapshot.battery_energy_kwh < opening_energy);
        assert_eq!(snapshot.unmet_load_kw, 0.0);
    }

    #[test]
    fn depleted_storage_sheds_flexible_load_and_reports_unserved_power() {
        let mut power = PowerSimulation::new();
        power.fission_capacity_kw = 0.0;
        power.battery_energy_kwh = 0.0;
        let snapshot = power.step(60.0, true, 0.0, 0.2, 12.0, 4.0, 4.5, 2.4, 1.5, 5.0, 2.0);
        assert!(snapshot.load_shed_active);
        assert_eq!(snapshot.flexible_load_kw, 13.0);
        assert_eq!(snapshot.requested_load_kw, snapshot.critical_load_kw);
        assert_eq!(snapshot.served_load_kw, 0.0);
        assert_eq!(snapshot.unmet_load_kw, snapshot.critical_load_kw);
        assert_eq!(snapshot.bus_voltage_v, 0.0);
        assert!(snapshot.alarm_bus_undervoltage);
    }

    #[test]
    fn eclss_oru_failure_reduces_physical_capacity_and_repair_restores_it() {
        let mut eclss = EclssSimulation::new();
        eclss
            .inject_component_failure("oga_water_assembly_oru")
            .unwrap();
        let failed = eclss.step(300.0, true);
        assert!(failed.o2_generation_kgph < 0.01);
        assert!(failed.alarm_maintenance_required);
        assert_eq!(failed.reliability.failed_count, 1);

        let work_order = eclss
            .start_component_repair("oga_water_assembly_oru")
            .unwrap();
        eclss
            .apply_component_repair_labor(&work_order.work_order_id, "oga_water_assembly_oru", 4.2)
            .unwrap();
        let repaired = eclss.step(1.0, true);
        assert_eq!(repaired.reliability.failed_count, 0);
        assert_eq!(repaired.reliability.repairing_count, 0);
        assert!(repaired.o2_generation_kgph > 0.0);
        assert_eq!(repaired.reliability.spares_remaining, 7);
    }

    #[test]
    fn degraded_cdra_produces_partial_not_binary_capacity() {
        let mut eclss = EclssSimulation::new();
        eclss
            .inject_component_degradation("cdra_desiccant_adsorbent_assembly", 25.0)
            .unwrap();
        let snapshot = eclss.step(10.0, true);
        let cdra = snapshot
            .reliability
            .components
            .iter()
            .find(|component| component.component_id == "cdra_desiccant_adsorbent_assembly")
            .unwrap();
        assert_eq!(cdra.capacity_fraction, 0.25);
        assert_eq!(snapshot.reliability.degraded_count, 1);
        assert!(snapshot.co2_capture_kgph > 0.0);
        assert!(snapshot.co2_capture_kgph < 0.7);
    }

    #[test]
    fn legacy_eclss_checkpoint_gains_a_valid_default_reliability_profile() {
        let mut value = serde_json::to_value(EclssSimulation::new()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("reliability");
        object.remove("sim_time_sec");

        let restored: EclssSimulation = serde_json::from_value(value).unwrap();
        let snapshot = restored.snapshot();
        assert_eq!(snapshot.sim_time_sec, 0.0);
        assert_eq!(snapshot.reliability.components.len(), 4);
        assert_eq!(snapshot.reliability.spares_remaining, 8);
    }

    #[test]
    fn power_state_round_trip_preserves_energy_and_integrals() {
        let mut power = PowerSimulation::new();
        for _ in 0..100 {
            power.step(1.0, true, 450.0, 0.2, 11.0, 4.0, 4.5, 2.4, 1.5, 4.0, 1.0);
        }
        let restored: PowerSimulation =
            serde_json::from_slice(&serde_json::to_vec(&power).unwrap()).unwrap();
        let before = power.snapshot();
        let after = restored.snapshot();
        assert!((after.battery_energy_kwh - before.battery_energy_kwh).abs() < 1.0e-12);
        assert!((after.cumulative_generated_kwh - before.cumulative_generated_kwh).abs() < 1.0e-12);
        assert!((after.cumulative_served_kwh - before.cumulative_served_kwh).abs() < 1.0e-12);
    }

    #[test]
    fn thermal_energy_balance_closes_each_step() {
        let mut thermal = ThermalSimulation::new();
        for _ in 0..20_000 {
            let snapshot = thermal.step(0.05, true, true, 12.0, 5.0, 0.0, 45.0, -63.0);
            assert!(snapshot.instantaneous_balance_error_kw.abs() < 1.0e-8);
            assert!(snapshot.stored_thermal_energy_mj.is_finite());
        }
    }

    #[test]
    fn loss_of_active_cooling_eventually_overheats_habitat() {
        let mut thermal = ThermalSimulation::new();
        for _ in 0..20_000 {
            thermal.step(1.0, false, false, 15.0, 6.0, 0.0, 50.0, -63.0);
        }
        let snapshot = thermal.snapshot();
        assert!(snapshot.habitat_temp_c > 30.0);
        assert!(snapshot.alarm_habitat_hot);
        assert!(!snapshot.cooling_available);
    }

    #[test]
    fn thermal_state_round_trip_preserves_energy_ledger() {
        let mut thermal = ThermalSimulation::new();
        for _ in 0..100 {
            thermal.step(1.0, true, true, 12.0, 5.0, 0.0, 45.0, -63.0);
        }
        let restored: ThermalSimulation =
            serde_json::from_slice(&serde_json::to_vec(&thermal).unwrap()).unwrap();
        let before = thermal.snapshot();
        let after = restored.snapshot();
        assert_eq!(
            before.stored_thermal_energy_mj,
            after.stored_thermal_energy_mj
        );
        assert_eq!(
            before.cumulative_heat_load_kwh,
            after.cumulative_heat_load_kwh
        );
        assert_eq!(
            before.cumulative_heat_rejected_kwh,
            after.cumulative_heat_rejected_kwh
        );
    }

    #[test]
    fn water_mass_balance_closes_through_treatment_and_consumption() {
        let mut water = WaterSimulation::new();
        for _ in 0..20_000 {
            let snapshot = water.step(0.5, true, true, 0.5, 0.4);
            assert!(snapshot.instantaneous_balance_error_kgph.abs() < 1.0e-7);
            assert!(snapshot.potable_water_kg >= 0.0);
            assert!(snapshot.wastewater_kg >= 0.0);
            assert!(snapshot.brine_kg >= 0.0);
        }
    }

    #[test]
    fn injected_conductivity_contamination_changes_physical_truth_and_persists() {
        let mut water = WaterSimulation::new();
        water.set_conductivity_contamination_target(850.0).unwrap();
        for _ in 0..120 {
            water.step(1.0, true, true, 0.5, 0.5);
        }
        let contaminated = water.snapshot();
        assert!(contaminated.potable_conductivity_us_cm > 500.0);
        assert!(contaminated.alarm_water_quality);

        let restored: WaterSimulation =
            serde_json::from_slice(&serde_json::to_vec(&water).unwrap()).unwrap();
        assert_eq!(restored.conductivity_contamination_target(), 850.0);
        assert_eq!(
            restored.snapshot().potable_conductivity_us_cm,
            contaminated.potable_conductivity_us_cm
        );
    }

    #[test]
    fn treatment_shutdown_degrades_quality_and_accumulates_wastewater() {
        let mut water = WaterSimulation::new();
        let opening_wastewater = water.snapshot().wastewater_kg;
        for _ in 0..43_200 {
            water.step(1.0, false, false, 0.5, 0.4);
        }
        let snapshot = water.snapshot();
        assert!(!snapshot.treatment_available);
        assert!(snapshot.wastewater_kg > opening_wastewater);
        assert!(snapshot.alarm_water_quality);
        assert!(snapshot.instantaneous_balance_error_kgph.abs() < 1.0e-7);
    }

    #[test]
    fn water_state_round_trip_preserves_stocks_and_integrals() {
        let mut water = WaterSimulation::new();
        for _ in 0..100 {
            water.step(1.0, true, true, 0.5, 0.4);
        }
        let restored: WaterSimulation =
            serde_json::from_slice(&serde_json::to_vec(&water).unwrap()).unwrap();
        let before = water.snapshot();
        let after = restored.snapshot();
        assert_eq!(before.potable_water_kg, after.potable_water_kg);
        assert_eq!(before.wastewater_kg, after.wastewater_kg);
        assert_eq!(before.brine_kg, after.brine_kg);
        assert_eq!(
            before.cumulative_external_input_kg,
            after.cumulative_external_input_kg
        );
    }

    #[test]
    fn safety_atmosphere_mass_balance_closes_during_leak_and_makeup() {
        let mut safety = SafetySimulation::new();
        safety.injected_leak_kg_s = 0.002;
        for _ in 0..10_000 {
            let snapshot = safety.step(0.05, true, true, true, 0.61, 0.025);
            assert!(snapshot.instantaneous_mass_balance_error_kgph.abs() < 1.0e-7);
            assert!(snapshot.habitat_air_mass_kg >= 0.0);
        }
        let snapshot = safety.snapshot();
        assert!(snapshot.cumulative_leaked_air_kg > 0.0);
        assert!(snapshot.cumulative_makeup_air_kg > 0.0);
    }

    #[test]
    fn safety_outage_does_not_pause_hazard_physics() {
        let mut safety = SafetySimulation::new();
        safety.injected_leak_kg_s = 0.01;
        safety.fire_source_kw = 30.0;
        let opening_pressure = safety.snapshot().habitat_pressure_kpa;
        for _ in 0..600 {
            safety.step(1.0, false, false, false, 0.61, 0.025);
        }
        let snapshot = safety.snapshot();
        assert!(!snapshot.monitoring_available);
        assert_eq!(snapshot.suppression_flow_kgph, 0.0);
        assert!(snapshot.habitat_pressure_kpa < opening_pressure);
        assert!(snapshot.smoke_ppm > 0.0);
        assert!(snapshot.carbon_monoxide_ppm > 25.0);
        assert!(snapshot.alarm_fire);
        assert!(snapshot.alarm_toxic_gas);
    }

    #[test]
    fn powered_safety_system_suppresses_a_fire_without_ending_the_plant() {
        let mut safety = SafetySimulation::new();
        safety.fire_source_kw = 30.0;
        let opening_agent = safety.suppression_agent_kg;
        for _ in 0..3_600 {
            safety.step(1.0, true, true, true, 0.61, 0.025);
        }
        let snapshot = safety.snapshot();
        assert!(snapshot.fire_heat_release_kw < 5.0);
        assert!(snapshot.suppression_agent_kg < opening_agent);
        assert!(snapshot.sim_time_sec >= 3_600.0);
    }

    #[test]
    fn safety_state_round_trip_preserves_exposure_and_inventory() {
        let mut safety = SafetySimulation::new();
        safety.injected_leak_kg_s = 0.001;
        for _ in 0..100 {
            safety.step(1.0, true, true, true, 0.61, 0.025);
        }
        let restored: SafetySimulation =
            serde_json::from_slice(&serde_json::to_vec(&safety).unwrap()).unwrap();
        let before = safety.snapshot();
        let after = restored.snapshot();
        assert_eq!(before.habitat_air_mass_kg, after.habitat_air_mass_kg);
        assert_eq!(before.suppression_agent_kg, after.suppression_agent_kg);
        assert_eq!(
            before.cumulative_internal_dose_msv,
            after.cumulative_internal_dose_msv
        );
    }

    #[test]
    fn safety_hazard_injection_rejects_unbounded_inputs() {
        let mut safety = SafetySimulation::new();
        assert!(safety.set_hazards(Some(-0.1), None, None).is_err());
        assert!(safety.set_hazards(None, Some(2_001.0), None).is_err());
        safety
            .set_hazards(Some(0.01), Some(25.0), Some(true))
            .unwrap();
        let snapshot = safety.snapshot();
        assert!(snapshot.habitat_isolated);
        assert_eq!(snapshot.fire_heat_release_kw, 25.0);
        assert!(snapshot.leak_rate_kgph > 36.0);
    }
}
