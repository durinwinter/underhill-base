use serde::{Deserialize, Serialize};

use crate::sim::Simulation;

pub const ENVIRONMENT_SCHEMA_VERSION: u32 = 1;
pub const MARS_SOL_SEC: f64 = 88_775.244;
const MARS_YEAR_SOLS: f64 = 668.6;

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentSnapshot {
    pub schema_version: u32,
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub mars_sol: u64,
    pub sol_fraction: f64,
    pub seasonal_phase: f64,
    pub solar_longitude_deg: f64,
    pub ambient_temperature_c: f64,
    pub exterior_pressure_kpa: f64,
    pub wind_speed_m_s: f64,
    pub wind_direction_deg: f64,
    pub dust_optical_depth: f64,
    pub dust_deposition_mg_m2_h: f64,
    pub top_of_atmosphere_solar_w_m2: f64,
    pub surface_solar_irradiance_w_m2: f64,
    pub external_radiation_msv_h: f64,
    pub monitoring_available: bool,
    pub alarm_dust_storm: bool,
    pub alarm_high_wind: bool,
    pub alarm_solar_particle_event: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSimulation {
    schema_version: u32,
    sim_time_sec: f64,
    site_latitude_deg: f64,
    site_elevation_m: f64,
    phase_seed: f64,
    ambient_temperature_c: f64,
    exterior_pressure_kpa: f64,
    wind_speed_m_s: f64,
    wind_direction_deg: f64,
    dust_optical_depth: f64,
    dust_deposition_mg_m2_h: f64,
    top_of_atmosphere_solar_w_m2: f64,
    surface_solar_irradiance_w_m2: f64,
    external_radiation_msv_h: f64,
    monitoring_available: bool,
}

impl Default for EnvironmentSimulation {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvironmentSimulation {
    pub fn new() -> Self {
        let mut simulation = Self {
            schema_version: ENVIRONMENT_SCHEMA_VERSION,
            sim_time_sec: 0.0,
            site_latitude_deg: -4.5,
            site_elevation_m: -2_600.0,
            phase_seed: 0.314_159_265_358_979_3,
            ambient_temperature_c: -55.0,
            exterior_pressure_kpa: 0.68,
            wind_speed_m_s: 4.0,
            wind_direction_deg: 210.0,
            dust_optical_depth: 0.25,
            dust_deposition_mg_m2_h: 0.0,
            top_of_atmosphere_solar_w_m2: 0.0,
            surface_solar_irradiance_w_m2: 0.0,
            external_radiation_msv_h: 0.025,
            monitoring_available: true,
        };
        simulation.update_conditions();
        simulation
    }

    pub fn step(&mut self, dt_sec: f64, monitoring_running: bool) -> EnvironmentSnapshot {
        if dt_sec.is_finite() && dt_sec > 0.0 {
            self.sim_time_sec += dt_sec;
            self.update_conditions();
        }
        self.monitoring_available = monitoring_running;
        self.snapshot()
    }

    pub fn snapshot(&self) -> EnvironmentSnapshot {
        let sol_position = self.sim_time_sec / MARS_SOL_SEC;
        let sol_fraction = sol_position.fract();
        let seasonal_phase = (sol_position / MARS_YEAR_SOLS).fract();
        EnvironmentSnapshot {
            schema_version: self.schema_version,
            timestamp_ms: Simulation::now_ms(),
            sim_time_sec: self.sim_time_sec,
            mars_sol: sol_position.floor() as u64,
            sol_fraction,
            seasonal_phase,
            solar_longitude_deg: seasonal_phase * 360.0,
            ambient_temperature_c: self.ambient_temperature_c,
            exterior_pressure_kpa: self.exterior_pressure_kpa,
            wind_speed_m_s: self.wind_speed_m_s,
            wind_direction_deg: self.wind_direction_deg,
            dust_optical_depth: self.dust_optical_depth,
            dust_deposition_mg_m2_h: self.dust_deposition_mg_m2_h,
            top_of_atmosphere_solar_w_m2: self.top_of_atmosphere_solar_w_m2,
            surface_solar_irradiance_w_m2: self.surface_solar_irradiance_w_m2,
            external_radiation_msv_h: self.external_radiation_msv_h,
            monitoring_available: self.monitoring_available,
            alarm_dust_storm: self.dust_optical_depth >= 1.0,
            alarm_high_wind: self.wind_speed_m_s >= 20.0,
            alarm_solar_particle_event: self.external_radiation_msv_h >= 0.2,
        }
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/EnvironmentService/ServiceInformation".to_string(),
            "ServiceSet/EnvironmentService/Modes".to_string(),
            "ServiceSet/EnvironmentService/StateMachine".to_string(),
            "ServiceSet/EnvironmentService/DataAssemblies/Atmosphere".to_string(),
            "ServiceSet/EnvironmentService/DataAssemblies/Solar".to_string(),
            "ServiceSet/EnvironmentService/DataAssemblies/Dust".to_string(),
            "ServiceSet/EnvironmentService/DataAssemblies/Radiation".to_string(),
            "ServiceSet/EnvironmentService/DataAssemblies/Alarms".to_string(),
        ]
    }

    fn update_conditions(&mut self) {
        let sol_position = self.sim_time_sec / MARS_SOL_SEC;
        let sol_fraction = sol_position.fract();
        let seasonal_phase = (sol_position / MARS_YEAR_SOLS).fract();
        let diurnal_angle = std::f64::consts::TAU * (sol_fraction - 0.5);
        let seasonal_angle = std::f64::consts::TAU * seasonal_phase;

        let daylight = diurnal_angle.cos().max(0.0);
        let orbital_flux_factor = 1.0 + 0.19 * seasonal_angle.cos();
        self.top_of_atmosphere_solar_w_m2 = 590.0 * orbital_flux_factor * daylight;

        let regional_cycle =
            (std::f64::consts::TAU * (sol_position / 43.0 + self.phase_seed)).sin();
        let synoptic_cycle =
            (std::f64::consts::TAU * (sol_position / 7.3 + self.phase_seed * 2.0)).sin();
        let southern_summer = ((seasonal_angle - 0.35).cos().max(0.0)).powi(4);
        let storm_intensity =
            ((regional_cycle - 0.58) / 0.42).clamp(0.0, 1.0) * (0.35 + 0.65 * southern_summer);
        self.dust_optical_depth =
            (0.22 + 0.06 * synoptic_cycle + 2.2 * storm_intensity).clamp(0.12, 3.0);
        self.surface_solar_irradiance_w_m2 =
            self.top_of_atmosphere_solar_w_m2 * (-self.dust_optical_depth).exp();

        self.wind_speed_m_s = (4.0
            + 3.5 * (diurnal_angle - 0.4).sin().abs()
            + 4.0 * synoptic_cycle.max(0.0)
            + 18.0 * storm_intensity)
            .clamp(0.0, 35.0);
        self.wind_direction_deg =
            (210.0 + 55.0 * synoptic_cycle + 90.0 * storm_intensity).rem_euclid(360.0);
        self.dust_deposition_mg_m2_h =
            self.wind_speed_m_s * self.dust_optical_depth * (0.06 + 0.3 * storm_intensity);

        let elevation_pressure_factor = (-self.site_elevation_m / 11_100.0).exp();
        self.exterior_pressure_kpa = (0.61
            * elevation_pressure_factor
            * (1.0 + 0.11 * (seasonal_angle - 0.8).sin())
            * (1.0 + 0.008 * synoptic_cycle))
            .clamp(0.45, 0.9);
        let latitude_factor = 1.0 - (self.site_latitude_deg.abs() / 90.0) * 0.12;
        self.ambient_temperature_c =
            -63.0 + 13.0 * seasonal_angle.cos() + 34.0 * diurnal_angle.cos() * latitude_factor
                - 5.0 * storm_intensity;

        let particle_event_phase = (sol_position + self.phase_seed * 10.0).rem_euclid(29.5);
        let particle_event = if particle_event_phase < 0.35 {
            let normalized = particle_event_phase / 0.35;
            0.75 * (std::f64::consts::PI * normalized).sin().max(0.0)
        } else {
            0.0
        };
        self.external_radiation_msv_h = 0.025 + 0.008 * seasonal_angle.sin().abs() + particle_event;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_is_cyclic_and_never_enters_a_terminal_state() {
        let mut environment = EnvironmentSimulation::new();
        let initial = environment.snapshot();
        let after_sol = environment.step(MARS_SOL_SEC, true);
        assert_eq!(after_sol.mars_sol, 1);
        assert!((after_sol.sol_fraction - initial.sol_fraction).abs() < 1.0e-12);
        assert!(after_sol.surface_solar_irradiance_w_m2.is_finite());
        assert!(after_sol.exterior_pressure_kpa > 0.0);
    }

    #[test]
    fn stopping_monitoring_does_not_stop_external_physics() {
        let mut environment = EnvironmentSimulation::new();
        let before = environment.snapshot();
        let stopped = environment.step(600.0, false);
        assert!(!stopped.monitoring_available);
        assert!(stopped.sim_time_sec > before.sim_time_sec);
        assert_ne!(stopped.ambient_temperature_c, before.ambient_temperature_c);
    }

    #[test]
    fn checkpoint_round_trip_preserves_weather_phase() {
        let mut environment = EnvironmentSimulation::new();
        environment.step(MARS_SOL_SEC * 17.25, true);
        let encoded = serde_json::to_vec(&environment).unwrap();
        let restored: EnvironmentSimulation = serde_json::from_slice(&encoded).unwrap();
        let expected = environment.snapshot();
        let actual = restored.snapshot();
        assert_eq!(actual.sim_time_sec, expected.sim_time_sec);
        assert_eq!(actual.dust_optical_depth, expected.dust_optical_depth);
        assert_eq!(
            actual.external_radiation_msv_h,
            expected.external_radiation_msv_h
        );
    }
}
