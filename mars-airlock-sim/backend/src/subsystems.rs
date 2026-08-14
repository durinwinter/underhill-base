use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
