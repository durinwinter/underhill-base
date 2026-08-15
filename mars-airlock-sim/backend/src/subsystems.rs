use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const MARS_SOL_SEC: f64 = 88_775.244;

#[derive(Debug, Clone, Serialize)]
pub struct EclssSnapshot {
    pub timestamp_ms: u64,
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

    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        eclss_load_kw: f64,
        sabatier_load_kw: f64,
        airlock_load_kw: f64,
    ) -> PowerSnapshot {
        self.sim_time_sec += dt_sec;
        let sol_fraction = (self.sim_time_sec % MARS_SOL_SEC) / MARS_SOL_SEC;
        let daylight = (std::f64::consts::TAU * sol_fraction).cos().max(0.0);
        self.solar_irradiance_w_m2 = 590.0 * daylight * (-self.dust_opacity).exp();
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

        self.critical_load_kw = 18.0 + eclss_load_kw.max(0.0) + airlock_load_kw.max(0.0);
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
pub struct EclssSimulation {
    cabin_pressure_kpa: f64,
    o2_percent: f64,
    co2_ppm: f64,
    humidity_pct: f64,
    water_recovery_pct: f64,
    co2_capture_kgph: f64,
    o2_generation_kgph: f64,
    power_kw: f64,
}

impl EclssSimulation {
    pub fn new() -> Self {
        Self {
            cabin_pressure_kpa: 101.3,
            o2_percent: 21.0,
            co2_ppm: 950.0,
            humidity_pct: 44.0,
            water_recovery_pct: 83.0,
            co2_capture_kgph: 0.7,
            o2_generation_kgph: 0.8,
            power_kw: 11.0,
        }
    }

    pub fn step(&mut self, dt_sec: f64, running: bool) -> EclssSnapshot {
        if running {
            let co2_target_capture = ((self.co2_ppm - 700.0) / 500.0).clamp(0.25, 1.8);
            self.co2_capture_kgph =
                first_order(self.co2_capture_kgph, co2_target_capture, 0.35, dt_sec);

            let o2_target_gen = ((21.1 - self.o2_percent) * 0.9 + 0.7).clamp(0.5, 1.6);
            self.o2_generation_kgph =
                first_order(self.o2_generation_kgph, o2_target_gen, 0.25, dt_sec);

            let crew_co2_load = 65.0;
            self.co2_ppm += dt_sec * (crew_co2_load - self.co2_capture_kgph * 80.0);
            self.co2_ppm = self.co2_ppm.clamp(400.0, 9000.0);

            self.o2_percent += dt_sec * (self.o2_generation_kgph * 0.015 - 0.004);
            self.o2_percent = self.o2_percent.clamp(18.0, 24.0);

            self.humidity_pct = first_order(self.humidity_pct, 47.0, 0.12, dt_sec);
            self.water_recovery_pct = first_order(self.water_recovery_pct, 88.0, 0.08, dt_sec);
            self.power_kw = first_order(
                self.power_kw,
                8.5 + self.co2_capture_kgph * 2.8 + self.o2_generation_kgph * 2.4,
                0.3,
                dt_sec,
            );
        } else {
            self.co2_capture_kgph = first_order(self.co2_capture_kgph, 0.0, 0.5, dt_sec);
            self.o2_generation_kgph = first_order(self.o2_generation_kgph, 0.0, 0.5, dt_sec);
            self.co2_ppm += dt_sec * 38.0;
            self.co2_ppm = self.co2_ppm.clamp(400.0, 9000.0);
            self.o2_percent = first_order(self.o2_percent, 20.4, 0.02, dt_sec);
            self.humidity_pct = first_order(self.humidity_pct, 54.0, 0.04, dt_sec);
            self.water_recovery_pct = first_order(self.water_recovery_pct, 64.0, 0.04, dt_sec);
            self.power_kw = first_order(self.power_kw, 2.0, 0.4, dt_sec);
        }

        self.snapshot()
    }

    pub fn snapshot(&self) -> EclssSnapshot {
        EclssSnapshot {
            timestamp_ms: now_ms(),
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
        }
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/EclssService/ServiceInformation".to_string(),
            "ServiceSet/EclssService/Modes".to_string(),
            "ServiceSet/EclssService/StateMachine".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Indicators".to_string(),
            "ServiceSet/EclssService/DataAssemblies/Parameters".to_string(),
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
        }
    }

    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        eclss_co2_available_kgph: f64,
    ) -> SabatierSnapshot {
        if running {
            self.reactor_temp_c = first_order(self.reactor_temp_c, 325.0, 0.18, dt_sec);
            self.reactor_pressure_bar = first_order(self.reactor_pressure_bar, 9.0, 0.18, dt_sec);

            let co2_target = eclss_co2_available_kgph.clamp(0.2, 1.6);
            self.co2_feed_kgph = first_order(self.co2_feed_kgph, co2_target, 0.3, dt_sec);
            self.h2_feed_kgph =
                first_order(self.h2_feed_kgph, self.co2_feed_kgph * 0.182, 0.25, dt_sec);

            let temperature_factor =
                1.0 - ((self.reactor_temp_c - 340.0).abs() / 220.0).clamp(0.0, 0.35);
            let catalyst_factor = (self.catalyst_health_pct / 100.0).clamp(0.65, 1.0);
            self.conversion_efficiency_pct =
                (84.0 * temperature_factor * catalyst_factor).clamp(60.0, 91.0);

            let efficiency = self.conversion_efficiency_pct / 100.0;
            self.methane_production_kgph = self.co2_feed_kgph * 0.364 * efficiency;
            self.water_production_kgph = self.co2_feed_kgph * 0.818 * efficiency;
            self.power_kw =
                first_order(self.power_kw, 3.4 + self.co2_feed_kgph * 1.9, 0.22, dt_sec);

            self.catalyst_health_pct -= dt_sec * 0.00065;
            self.catalyst_health_pct = self.catalyst_health_pct.clamp(70.0, 100.0);
        } else {
            self.reactor_temp_c = first_order(self.reactor_temp_c, 215.0, 0.22, dt_sec);
            self.reactor_pressure_bar = first_order(self.reactor_pressure_bar, 3.5, 0.22, dt_sec);
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
            let snapshot = power.step(0.05, true, 12.0, 5.0, 2.0);
            assert!(snapshot.instantaneous_balance_error_kw.abs() < 1.0e-9);
            assert!(snapshot.battery_energy_kwh >= 0.0);
            assert!(snapshot.battery_energy_kwh <= 500.0);
        }
    }

    #[test]
    fn battery_discharges_during_martian_night() {
        let mut power = PowerSimulation::new();
        power.sim_time_sec = MARS_SOL_SEC * 0.5;
        let opening_energy = power.battery_energy_kwh;
        let snapshot = power.step(60.0, true, 12.0, 5.0, 2.0);
        assert_eq!(snapshot.solar_available_kw, 0.0);
        assert!(snapshot.battery_power_kw < 0.0);
        assert!(snapshot.battery_energy_kwh < opening_energy);
        assert_eq!(snapshot.unmet_load_kw, 0.0);
    }

    #[test]
    fn depleted_storage_sheds_flexible_load_and_reports_unserved_power() {
        let mut power = PowerSimulation::new();
        power.sim_time_sec = MARS_SOL_SEC * 0.5;
        power.fission_capacity_kw = 0.0;
        power.battery_energy_kwh = 0.0;
        let snapshot = power.step(60.0, true, 12.0, 5.0, 2.0);
        assert!(snapshot.load_shed_active);
        assert_eq!(snapshot.flexible_load_kw, 13.0);
        assert_eq!(snapshot.requested_load_kw, snapshot.critical_load_kw);
        assert_eq!(snapshot.served_load_kw, 0.0);
        assert_eq!(snapshot.unmet_load_kw, snapshot.critical_load_kw);
        assert_eq!(snapshot.bus_voltage_v, 0.0);
        assert!(snapshot.alarm_bus_undervoltage);
    }

    #[test]
    fn power_state_round_trip_preserves_energy_and_integrals() {
        let mut power = PowerSimulation::new();
        for _ in 0..100 {
            power.step(1.0, true, 11.0, 4.0, 1.0);
        }
        let restored: PowerSimulation =
            serde_json::from_slice(&serde_json::to_vec(&power).unwrap()).unwrap();
        let before = power.snapshot();
        let after = restored.snapshot();
        assert_eq!(after.battery_energy_kwh, before.battery_energy_kwh);
        assert_eq!(
            after.cumulative_generated_kwh,
            before.cumulative_generated_kwh
        );
        assert_eq!(after.cumulative_served_kwh, before.cumulative_served_kwh);
    }
}
