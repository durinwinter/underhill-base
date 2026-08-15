use std::env;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const DEFAULT_FIXED_STEP_SEC: f64 = 0.05;
pub const DEFAULT_WALL_TICK_MS: u64 = 50;
pub const MARS_SOL_SEC: f64 = 88_775.244;

const DEFAULT_MEDIUM_PERIOD_SEC: f64 = 1.0;
const DEFAULT_SLOW_PERIOD_SEC: f64 = 60.0;
const DEFAULT_MAX_STEPS_PER_WALL_TICK: u64 = 2_000;
const SCHEDULER_EPSILON_SEC: f64 = 1.0e-9;

#[derive(Debug, Clone, Copy)]
pub struct PlantRuntimeConfig {
    pub fixed_step_sec: f64,
    pub wall_tick_ms: u64,
    pub time_scale: f64,
    pub medium_period_sec: f64,
    pub slow_period_sec: f64,
    pub max_steps_per_wall_tick: u64,
}

impl PlantRuntimeConfig {
    pub fn from_env() -> Result<Self> {
        let config = Self {
            fixed_step_sec: DEFAULT_FIXED_STEP_SEC,
            wall_tick_ms: parse_env_u64("UNDERHILL_WALL_TICK_MS", DEFAULT_WALL_TICK_MS)?,
            time_scale: parse_env_f64("UNDERHILL_TIME_SCALE", 1.0)?,
            medium_period_sec: parse_env_f64(
                "UNDERHILL_MEDIUM_PERIOD_SEC",
                DEFAULT_MEDIUM_PERIOD_SEC,
            )?,
            slow_period_sec: parse_env_f64("UNDERHILL_SLOW_PERIOD_SEC", DEFAULT_SLOW_PERIOD_SEC)?,
            max_steps_per_wall_tick: parse_env_u64(
                "UNDERHILL_MAX_STEPS_PER_WALL_TICK",
                DEFAULT_MAX_STEPS_PER_WALL_TICK,
            )?,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        validate_positive_finite("UNDERHILL_FIXED_STEP_SEC", self.fixed_step_sec)?;
        validate_positive_finite("UNDERHILL_TIME_SCALE", self.time_scale)?;
        validate_positive_finite("UNDERHILL_MEDIUM_PERIOD_SEC", self.medium_period_sec)?;
        validate_positive_finite("UNDERHILL_SLOW_PERIOD_SEC", self.slow_period_sec)?;
        if self.wall_tick_ms == 0 {
            bail!("UNDERHILL_WALL_TICK_MS must be greater than zero");
        }
        if self.max_steps_per_wall_tick == 0 {
            bail!("UNDERHILL_MAX_STEPS_PER_WALL_TICK must be greater than zero");
        }
        if self.medium_period_sec + SCHEDULER_EPSILON_SEC < self.fixed_step_sec {
            bail!("UNDERHILL_MEDIUM_PERIOD_SEC must not be shorter than the fixed step");
        }
        if self.slow_period_sec + SCHEDULER_EPSILON_SEC < self.medium_period_sec {
            bail!("UNDERHILL_SLOW_PERIOD_SEC must not be shorter than the medium period");
        }
        Ok(())
    }

    pub fn mode(self) -> PlantTimeMode {
        if (self.time_scale - 1.0).abs() < f64::EPSILON {
            PlantTimeMode::Live
        } else {
            PlantTimeMode::Accelerated
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantTimeMode {
    Live,
    Accelerated,
}

#[derive(Debug, Clone, Copy)]
pub struct ScheduledStep {
    pub fixed_step_sec: f64,
    pub run_medium: bool,
    pub run_slow: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlantRuntimeSnapshot {
    pub mode: PlantTimeMode,
    pub time_scale: f64,
    pub fixed_step_sec: f64,
    pub wall_tick_ms: u64,
    pub step_index: u64,
    pub plant_elapsed_sec: f64,
    pub mars_sol: u64,
    pub sol_fraction: f64,
    pub backlog_steps: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlantSchedulerState {
    pub accumulated_sim_sec: f64,
    pub plant_elapsed_sec: f64,
    pub step_index: u64,
}

impl PlantRuntimeSnapshot {
    pub fn new(config: PlantRuntimeConfig) -> Self {
        Self {
            mode: config.mode(),
            time_scale: config.time_scale,
            fixed_step_sec: config.fixed_step_sec,
            wall_tick_ms: config.wall_tick_ms,
            step_index: 0,
            plant_elapsed_sec: 0.0,
            mars_sol: 0,
            sol_fraction: 0.0,
            backlog_steps: 0,
        }
    }
}

pub struct PlantScheduler {
    config: PlantRuntimeConfig,
    accumulated_sim_sec: f64,
    plant_elapsed_sec: f64,
    step_index: u64,
    next_medium_sec: f64,
    next_slow_sec: f64,
}

impl PlantScheduler {
    pub fn new(config: PlantRuntimeConfig) -> Self {
        Self {
            config,
            accumulated_sim_sec: 0.0,
            plant_elapsed_sec: 0.0,
            step_index: 0,
            next_medium_sec: config.medium_period_sec,
            next_slow_sec: config.slow_period_sec,
        }
    }

    pub fn from_state(config: PlantRuntimeConfig, state: PlantSchedulerState) -> Result<Self> {
        config.validate()?;
        if !state.accumulated_sim_sec.is_finite() || state.accumulated_sim_sec < 0.0 {
            bail!("checkpoint scheduler accumulator must be finite and non-negative");
        }
        if !state.plant_elapsed_sec.is_finite() || state.plant_elapsed_sec < 0.0 {
            bail!("checkpoint plant elapsed time must be finite and non-negative");
        }
        Ok(Self {
            config,
            accumulated_sim_sec: state.accumulated_sim_sec,
            plant_elapsed_sec: state.plant_elapsed_sec,
            step_index: state.step_index,
            next_medium_sec: next_boundary(state.plant_elapsed_sec, config.medium_period_sec),
            next_slow_sec: next_boundary(state.plant_elapsed_sec, config.slow_period_sec),
        })
    }

    pub fn wall_tick_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.config.wall_tick_ms)
    }

    pub fn begin_wall_tick(&mut self) -> u64 {
        let wall_tick_sec = self.config.wall_tick_ms as f64 / 1_000.0;
        self.accumulated_sim_sec += wall_tick_sec * self.config.time_scale;
        let steps_due = ((self.accumulated_sim_sec + SCHEDULER_EPSILON_SEC)
            / self.config.fixed_step_sec)
            .floor() as u64;
        steps_due.min(self.config.max_steps_per_wall_tick)
    }

    pub fn advance_fixed_step(&mut self) -> ScheduledStep {
        self.accumulated_sim_sec = (self.accumulated_sim_sec - self.config.fixed_step_sec).max(0.0);
        self.plant_elapsed_sec += self.config.fixed_step_sec;
        self.step_index = self.step_index.wrapping_add(1);

        let run_medium = self.plant_elapsed_sec + SCHEDULER_EPSILON_SEC >= self.next_medium_sec;
        if run_medium {
            while self.next_medium_sec <= self.plant_elapsed_sec + SCHEDULER_EPSILON_SEC {
                self.next_medium_sec += self.config.medium_period_sec;
            }
        }

        let run_slow = self.plant_elapsed_sec + SCHEDULER_EPSILON_SEC >= self.next_slow_sec;
        if run_slow {
            while self.next_slow_sec <= self.plant_elapsed_sec + SCHEDULER_EPSILON_SEC {
                self.next_slow_sec += self.config.slow_period_sec;
            }
        }

        ScheduledStep {
            fixed_step_sec: self.config.fixed_step_sec,
            run_medium,
            run_slow,
        }
    }

    pub fn snapshot(&self) -> PlantRuntimeSnapshot {
        let mars_sol = (self.plant_elapsed_sec / MARS_SOL_SEC).floor() as u64;
        let sol_fraction = (self.plant_elapsed_sec % MARS_SOL_SEC) / MARS_SOL_SEC;
        PlantRuntimeSnapshot {
            mode: self.config.mode(),
            time_scale: self.config.time_scale,
            fixed_step_sec: self.config.fixed_step_sec,
            wall_tick_ms: self.config.wall_tick_ms,
            step_index: self.step_index,
            plant_elapsed_sec: self.plant_elapsed_sec,
            mars_sol,
            sol_fraction,
            backlog_steps: ((self.accumulated_sim_sec + SCHEDULER_EPSILON_SEC)
                / self.config.fixed_step_sec)
                .floor() as u64,
        }
    }

    pub fn state(&self) -> PlantSchedulerState {
        PlantSchedulerState {
            accumulated_sim_sec: self.accumulated_sim_sec,
            plant_elapsed_sec: self.plant_elapsed_sec,
            step_index: self.step_index,
        }
    }
}

fn next_boundary(elapsed_sec: f64, period_sec: f64) -> f64 {
    ((elapsed_sec / period_sec).floor() + 1.0) * period_sec
}

fn parse_env_f64(name: &str, default: f64) -> Result<f64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<f64>()
            .with_context(|| format!("Invalid {name} value {value}; expected a number")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(err).with_context(|| format!("Unable to read {name}")),
    }
}

fn parse_env_u64(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .with_context(|| format!("Invalid {name} value {value}; expected an integer")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(err).with_context(|| format!("Unable to read {name}")),
    }
}

fn validate_positive_finite(name: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        bail!("{name} must be a finite value greater than zero");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(time_scale: f64) -> PlantRuntimeConfig {
        PlantRuntimeConfig {
            fixed_step_sec: 0.05,
            wall_tick_ms: 50,
            time_scale,
            medium_period_sec: 1.0,
            slow_period_sec: 60.0,
            max_steps_per_wall_tick: 2_000,
        }
    }

    #[test]
    fn live_mode_advances_one_fixed_step_per_wall_tick() {
        let mut scheduler = PlantScheduler::new(config(1.0));
        assert_eq!(scheduler.begin_wall_tick(), 1);
        let step = scheduler.advance_fixed_step();
        let snapshot = scheduler.snapshot();
        assert_eq!(snapshot.step_index, 1);
        assert!((snapshot.plant_elapsed_sec - 0.05).abs() < 1.0e-12);
        assert!(!step.run_medium);
        assert!(!step.run_slow);
        assert_eq!(scheduler.snapshot().backlog_steps, 0);
    }

    #[test]
    fn accelerated_mode_preserves_fixed_physics_steps() {
        let mut scheduler = PlantScheduler::new(config(10.0));
        let steps = scheduler.begin_wall_tick();
        assert_eq!(steps, 10);
        for _ in 0..steps {
            assert!((scheduler.advance_fixed_step().fixed_step_sec - 0.05).abs() < 1.0e-12);
        }
        let snapshot = scheduler.snapshot();
        assert_eq!(snapshot.step_index, 10);
        assert!((snapshot.plant_elapsed_sec - 0.5).abs() < 1.0e-12);
        assert_eq!(snapshot.mode, PlantTimeMode::Accelerated);
    }

    #[test]
    fn medium_and_slow_tiers_fire_on_deterministic_boundaries() {
        let mut scheduler = PlantScheduler::new(config(1.0));
        let mut medium_steps = Vec::new();
        let mut slow_steps = Vec::new();
        for _ in 0..1_200 {
            assert_eq!(scheduler.begin_wall_tick(), 1);
            let step = scheduler.advance_fixed_step();
            if step.run_medium {
                medium_steps.push(scheduler.snapshot().step_index);
            }
            if step.run_slow {
                slow_steps.push(scheduler.snapshot().step_index);
            }
        }
        assert_eq!(medium_steps.len(), 60);
        assert_eq!(medium_steps[0], 20);
        assert_eq!(slow_steps, vec![1_200]);
    }

    #[test]
    fn catch_up_limit_retains_backlog_for_later_ticks() {
        let mut limited = config(10_000.0);
        limited.max_steps_per_wall_tick = 100;
        let mut scheduler = PlantScheduler::new(limited);
        assert_eq!(scheduler.begin_wall_tick(), 100);
        for _ in 0..100 {
            scheduler.advance_fixed_step();
        }
        assert_eq!(scheduler.snapshot().backlog_steps, 9_900);
    }

    #[test]
    fn restored_scheduler_continues_without_replaying_a_cadence_boundary() {
        let mut scheduler = PlantScheduler::new(config(1.0));
        for _ in 0..20 {
            assert_eq!(scheduler.begin_wall_tick(), 1);
            scheduler.advance_fixed_step();
        }
        let state = scheduler.state();
        let mut restored = PlantScheduler::from_state(config(1.0), state).unwrap();
        assert_eq!(restored.begin_wall_tick(), 1);
        let step = restored.advance_fixed_step();
        assert!(!step.run_medium);
        assert_eq!(restored.snapshot().step_index, 21);
    }
}
