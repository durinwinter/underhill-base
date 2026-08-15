#[cfg(test)]
mod dataset_manifest;
mod historian;
mod model;
mod mqtt_uns;
mod opcua;
mod opcua_subsystems;
mod pea_registry;
mod persistence;
mod plant_runtime;
mod sim;
mod subsystems;
mod tag_catalog;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::ErrorKind,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Router,
    extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade, ws::Message},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use model::{
    CommandEnum, CommandRequestFields, CommandResponseFields, CommandSourceEnum, EventEntry,
    LeakRateUpdateRequest, MtpModesUpdateRequest, OperationMode, ParameterCategory,
    PermissionsUpdateRequest, ProcedureRequest, ProcedureRequestInput, ProcedureState,
    ProcedureStatusResponse, SecurityProfileRequest, ServiceDefinition, ServiceParameter,
    ServiceProcedure, Snapshot, ValveFaultUpdateRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock, broadcast, watch},
    time,
};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{error, info, warn};
use zenoh::Session;

use crate::historian::{Historian, NewHistorianSample, TelemetryQuality};
use crate::pea_registry::{
    ALL_PEA_DEFINITIONS, DEFAULT_AIRLOCK_PEA_ID, DEFAULT_ECLSS_PEA_ID, DEFAULT_POWER_PEA_ID,
    DEFAULT_SABATIER_PEA_ID, DEFAULT_SAFETY_PEA_ID, DEFAULT_THERMAL_PEA_ID, DEFAULT_WATER_PEA_ID,
    ECLSS_SERVICE_TAG, POWER_SERVICE_TAG, SABATIER_SERVICE_TAG, SAFETY_SERVICE_TAG,
    THERMAL_SERVICE_TAG, WATER_SERVICE_TAG, definition_for,
};
use crate::persistence::{PlantCheckpoint, PlantPersistence, wall_time_ms};
use crate::plant_runtime::{
    DowntimePolicy, PlantRuntimeConfig, PlantRuntimeSnapshot, PlantScheduler, PlantSchedulerState,
};
use crate::sim::Simulation;
use crate::subsystems::{
    EclssSimulation, EclssSnapshot, PowerSimulation, PowerSnapshot, SabatierSimulation,
    SabatierSnapshot, SafetySimulation, SafetySnapshot, ThermalSimulation, ThermalSnapshot,
    WaterSimulation, WaterSnapshot,
};
use crate::tag_catalog::{CanonicalTag, TagCatalog};

const DEFAULT_NODE_ID: &str = "local";
const DEFAULT_OPCUA_PORT_RANGE_MIN: u16 = 4841;
const DEFAULT_OPCUA_PORT_RANGE_MAX: u16 = 4899;

/// Combined WebSocket snapshot for 3D visualization frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SystemsSnapshot {
    timestamp_ms: u64,
    // Airlock Chamber
    chamber_pressure: f64,
    inner_door_open: bool,
    outer_door_open: bool,
    // ECLSS System
    eclss_pressure: f64,
    eclss_status: String,
    co2_level: f64,
    o2_level: f64,
    thermal_load: f64,
    humidity: f64,
    // Sabatier Reactor
    reactor_temp: f64,
    reactor_pressure: f64,
    h2_flow: f64,
    co2_flow: f64,
    product_flow: f64,
    // Power System
    power_generation_kw: f64,
    power_load_kw: f64,
    battery_soc_pct: f64,
    dc_bus_voltage_v: f64,
    power_load_shed_active: bool,
    habitat_temp_c: f64,
    thermal_heat_rejection_kw: f64,
    thermal_cooling_available: bool,
    potable_water_kg: f64,
    wastewater_kg: f64,
    water_quality_alarm: bool,
    habitat_pressure_kpa: f64,
    internal_radiation_msv_h: f64,
    safety_alarm_active: bool,
    // Status
    healthy: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct PlantRecoveryStatus {
    restored_from_checkpoint: bool,
    downtime_policy: DowntimePolicy,
    checkpoint_age_sec: f64,
    queued_catchup_sec: f64,
    journal_tail_records: usize,
    unapplied_state_records: usize,
}

#[derive(Clone)]
struct AppContext {
    sim: Arc<RwLock<Simulation>>,
    airlock_runtime: Arc<RwLock<PeaRuntimeState>>,
    eclss_runtime: Arc<RwLock<PeaRuntimeState>>,
    sabatier_runtime: Arc<RwLock<PeaRuntimeState>>,
    power_runtime: Arc<RwLock<PeaRuntimeState>>,
    thermal_runtime: Arc<RwLock<PeaRuntimeState>>,
    water_runtime: Arc<RwLock<PeaRuntimeState>>,
    safety_runtime: Arc<RwLock<PeaRuntimeState>>,
    eclss_operator_state: Arc<RwLock<SubsystemOperatorState>>,
    sabatier_operator_state: Arc<RwLock<SubsystemOperatorState>>,
    power_operator_state: Arc<RwLock<SubsystemOperatorState>>,
    thermal_operator_state: Arc<RwLock<SubsystemOperatorState>>,
    water_operator_state: Arc<RwLock<SubsystemOperatorState>>,
    safety_operator_state: Arc<RwLock<SubsystemOperatorState>>,
    eclss_sim: Arc<RwLock<EclssSimulation>>,
    sabatier_sim: Arc<RwLock<SabatierSimulation>>,
    power_sim: Arc<RwLock<PowerSimulation>>,
    thermal_sim: Arc<RwLock<ThermalSimulation>>,
    water_sim: Arc<RwLock<WaterSimulation>>,
    safety_sim: Arc<RwLock<SafetySimulation>>,
    pea_opcua_endpoints: Arc<HashMap<String, String>>,
    zenoh_session: Option<Arc<Session>>,
    mqtt_uns: Option<Arc<mqtt_uns::MqttUnsPublisher>>,
    node_id: String,
    snapshots_tx: broadcast::Sender<Snapshot>,
    systems_snapshots_tx: broadcast::Sender<SystemsSnapshot>,
    plant_runtime: Arc<RwLock<PlantRuntimeSnapshot>>,
    plant_persistence: Arc<PlantPersistence>,
    plant_recovery: PlantRecoveryStatus,
    plant_transaction: Arc<Mutex<()>>,
    opcua_control: opcua::OpcuaControl,
    next_client_id: Arc<AtomicU64>,
    tag_catalog: Arc<TagCatalog>,
    historian: Arc<Historian>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct PeaRuntimeState {
    pub(crate) deployed: bool,
    pub(crate) running: bool,
    pub(crate) last_transition_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubsystemOperatorState {
    operation_mode: OperationMode,
    source_mode: CommandSourceEnum,
    command_en: bool,
    command_en_reason: String,
    operator_control_enabled: bool,
    remote_control_enabled: bool,
}

impl Default for SubsystemOperatorState {
    fn default() -> Self {
        Self {
            operation_mode: OperationMode::Auto,
            source_mode: CommandSourceEnum::OperatorUi,
            command_en: true,
            command_en_reason: String::new(),
            operator_control_enabled: true,
            remote_control_enabled: true,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PeaServiceCommandRequest {
    source: Option<String>,
    sequence_id: u32,
    command: CommandEnum,
    #[serde(default)]
    param1: f64,
    #[serde(default)]
    param2: f64,
    #[serde(default = "default_execute_true")]
    execute: bool,
}

#[derive(Debug, Deserialize)]
struct SubsystemOperatorStateUpdateRequest {
    operation_mode: Option<OperationMode>,
    source_mode: Option<CommandSourceEnum>,
    command_en: Option<bool>,
    command_en_reason: Option<String>,
    operator_control_enabled: Option<bool>,
    remote_control_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TelemetryCatalogQuery {
    offset: Option<usize>,
    limit: Option<usize>,
    subsystem_family: Option<String>,
    owner_pea: Option<String>,
    publication_class: Option<String>,
    activation_state: Option<String>,
}

#[derive(Debug, Serialize)]
struct TelemetryCatalogPage {
    schema_version: u32,
    total_matching: usize,
    offset: usize,
    limit: usize,
    next_offset: Option<usize>,
    items: Vec<CanonicalTag>,
}

#[derive(Debug, Deserialize)]
struct TelemetryHistoryQuery {
    tag_id: Option<String>,
    since_ms: Option<u64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SafetyHazardUpdateRequest {
    injected_leak_kg_s: Option<f64>,
    fire_source_kw: Option<f64>,
    habitat_isolated: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpcuaPortAllocationStore {
    version: u32,
    allocations: BTreeMap<String, u16>,
}

impl Default for OpcuaPortAllocationStore {
    fn default() -> Self {
        Self {
            version: 1,
            allocations: BTreeMap::new(),
        }
    }
}

fn default_execute_true() -> bool {
    true
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let plant_runtime_config = PlantRuntimeConfig::from_env()?;
    info!(
        "Underhill plant runtime mode={:?} time_scale={} fixed_step_sec={} wall_tick_ms={}",
        plant_runtime_config.mode(),
        plant_runtime_config.time_scale,
        plant_runtime_config.fixed_step_sec,
        plant_runtime_config.wall_tick_ms
    );

    let initial_security = std::env::var("AIRLOCK_SECURITY_PROFILE").unwrap_or("NONE".to_string());
    let forced_airlock_port = std::env::var("AIRLOCK_OPCUA_PORT")
        .ok()
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("Invalid AIRLOCK_OPCUA_PORT value {value}; expected integer")
            })
        })
        .transpose()?;
    let forced_eclss_port = std::env::var("ECLSS_OPCUA_PORT")
        .ok()
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("Invalid ECLSS_OPCUA_PORT value {value}; expected integer")
            })
        })
        .transpose()?;
    let forced_sabatier_port = std::env::var("SABATIER_OPCUA_PORT")
        .ok()
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("Invalid SABATIER_OPCUA_PORT value {value}; expected integer")
            })
        })
        .transpose()?;
    let forced_power_port = std::env::var("POWERGRID_OPCUA_PORT")
        .ok()
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("Invalid POWERGRID_OPCUA_PORT value {value}; expected integer")
            })
        })
        .transpose()?;
    let forced_thermal_port = std::env::var("THERMAL_OPCUA_PORT")
        .ok()
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("Invalid THERMAL_OPCUA_PORT value {value}; expected integer")
            })
        })
        .transpose()?;
    let forced_water_port = std::env::var("WATER_OPCUA_PORT")
        .ok()
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("Invalid WATER_OPCUA_PORT value {value}; expected integer")
            })
        })
        .transpose()?;
    let forced_safety_port = std::env::var("SAFETY_OPCUA_PORT")
        .ok()
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("Invalid SAFETY_OPCUA_PORT value {value}; expected integer")
            })
        })
        .transpose()?;

    let airlock_port = allocate_opcua_port_for_pea(DEFAULT_AIRLOCK_PEA_ID, forced_airlock_port)?;
    let eclss_port = allocate_opcua_port_for_pea(DEFAULT_ECLSS_PEA_ID, forced_eclss_port)?;
    let sabatier_port = allocate_opcua_port_for_pea(DEFAULT_SABATIER_PEA_ID, forced_sabatier_port)?;
    let power_port = allocate_opcua_port_for_pea(DEFAULT_POWER_PEA_ID, forced_power_port)?;
    let thermal_port = allocate_opcua_port_for_pea(DEFAULT_THERMAL_PEA_ID, forced_thermal_port)?;
    let water_port = allocate_opcua_port_for_pea(DEFAULT_WATER_PEA_ID, forced_water_port)?;
    let safety_port = allocate_opcua_port_for_pea(DEFAULT_SAFETY_PEA_ID, forced_safety_port)?;

    let opcua_runtime_config = opcua::OpcuaRuntimeConfig::from_env_with_port(airlock_port);
    let opcua_endpoint_url = opcua_runtime_config.endpoint_url();
    let opcua_host = resolve_opcua_advertised_host();
    let eclss_endpoint_url = build_opcua_endpoint_url(&opcua_host, eclss_port, "/underhill/eclss");
    let sabatier_endpoint_url =
        build_opcua_endpoint_url(&opcua_host, sabatier_port, "/underhill/sabatier");
    let power_endpoint_url = build_opcua_endpoint_url(&opcua_host, power_port, "/underhill/power");
    let thermal_endpoint_url =
        build_opcua_endpoint_url(&opcua_host, thermal_port, "/underhill/thermal");
    let water_endpoint_url = build_opcua_endpoint_url(&opcua_host, water_port, "/underhill/water");
    let safety_endpoint_url =
        build_opcua_endpoint_url(&opcua_host, safety_port, "/underhill/safety");
    info!(
        "Allocated OPC UA port {} for {} (endpoint {})",
        opcua_runtime_config.port(),
        DEFAULT_AIRLOCK_PEA_ID,
        opcua_endpoint_url
    );
    info!(
        "Reserved OPC UA ports {} ({}), {} ({}), {} ({}), {} ({}), {} ({}), and {} ({})",
        eclss_port,
        DEFAULT_ECLSS_PEA_ID,
        sabatier_port,
        DEFAULT_SABATIER_PEA_ID,
        power_port,
        DEFAULT_POWER_PEA_ID,
        thermal_port,
        DEFAULT_THERMAL_PEA_ID,
        water_port,
        DEFAULT_WATER_PEA_ID,
        safety_port,
        DEFAULT_SAFETY_PEA_ID
    );

    let plant_persistence = Arc::new(PlantPersistence::from_env()?);
    let historian = Arc::new(Historian::open(plant_persistence.state_dir())?);
    let restored_checkpoint = plant_persistence.load_checkpoint()?;
    let node_id = std::env::var("MURPH_NODE_ID").unwrap_or_else(|_| DEFAULT_NODE_ID.to_string());
    let zenoh_session = match std::env::var("ZENOH_ROUTER") {
        Ok(endpoint) if !endpoint.trim().is_empty() => match open_zenoh_session().await {
            Ok(session) => {
                info!("Connected Underhill backend to Zenoh router {endpoint}");
                Some(Arc::new(session))
            }
            Err(err) => {
                warn!("Zenoh unavailable, continuing without Zenoh UNS publishing: {err}");
                None
            }
        },
        _ => {
            info!("ZENOH_ROUTER is unset; Zenoh UNS publishing disabled");
            None
        }
    };
    let mqtt_uns = mqtt_uns::MqttUnsPublisher::from_env().await.map(Arc::new);
    let initial_transition_ms = Simulation::now_ms();
    let default_runtime = PeaRuntimeState {
        deployed: true,
        running: true,
        last_transition_ms: initial_transition_ms,
    };
    let (
        airlock_state,
        eclss_state,
        sabatier_state,
        power_state,
        thermal_state,
        water_state,
        safety_state,
        airlock_runtime_state,
        mut eclss_runtime_state,
        mut sabatier_runtime_state,
        mut power_runtime_state,
        mut thermal_runtime_state,
        mut water_runtime_state,
        mut safety_runtime_state,
        eclss_operator_state_value,
        sabatier_operator_state_value,
        power_operator_state_value,
        thermal_operator_state_value,
        water_operator_state_value,
        safety_operator_state_value,
        plant_scheduler,
        plant_recovery,
    ) = match restored_checkpoint {
        Some(mut checkpoint) => {
            let journal_tail =
                plant_persistence.journal_records_after(checkpoint.journal_sequence)?;
            let unapplied_state_records = journal_tail
                .iter()
                .filter(|record| is_state_journal_kind(&record.kind))
                .count();
            if unapplied_state_records > 0 {
                warn!(
                    "Checkpoint has {} durable state record(s) in its journal tail; deterministic state replay is required before this boundary is production-safe",
                    unapplied_state_records
                );
            }
            checkpoint
                .airlock
                .prepare_after_restore(opcua_endpoint_url.clone());
            let checkpoint_age_sec =
                wall_time_ms().saturating_sub(checkpoint.saved_wall_time_ms) as f64 / 1_000.0;
            let mut scheduler =
                PlantScheduler::from_state(plant_runtime_config, checkpoint.scheduler)?;
            let queued_catchup_sec = match plant_runtime_config.downtime_policy {
                DowntimePolicy::CatchUp => {
                    scheduler.queue_downtime_catchup(checkpoint_age_sec)?;
                    checkpoint_age_sec
                }
                DowntimePolicy::Freeze => 0.0,
            };
            info!(
                "Restored plant {} at simulated second {}; downtime_policy={} checkpoint_age_sec={:.3} queued_catchup_sec={:.3}",
                plant_persistence.plant_id(),
                checkpoint.scheduler.plant_elapsed_sec,
                plant_runtime_config.downtime_policy.as_str(),
                checkpoint_age_sec,
                queued_catchup_sec
            );
            (
                checkpoint.airlock,
                checkpoint.eclss,
                checkpoint.sabatier,
                checkpoint.power,
                checkpoint.thermal,
                checkpoint.water,
                checkpoint.safety,
                checkpoint.airlock_runtime,
                checkpoint.eclss_runtime,
                checkpoint.sabatier_runtime,
                checkpoint.power_runtime,
                checkpoint.thermal_runtime,
                checkpoint.water_runtime,
                checkpoint.safety_runtime,
                checkpoint.eclss_operator_state,
                checkpoint.sabatier_operator_state,
                checkpoint.power_operator_state,
                checkpoint.thermal_operator_state,
                checkpoint.water_operator_state,
                checkpoint.safety_operator_state,
                scheduler,
                PlantRecoveryStatus {
                    restored_from_checkpoint: true,
                    downtime_policy: plant_runtime_config.downtime_policy,
                    checkpoint_age_sec,
                    queued_catchup_sec,
                    journal_tail_records: journal_tail.len(),
                    unapplied_state_records,
                },
            )
        }
        None => (
            Simulation::new(initial_security.clone(), opcua_endpoint_url.clone()),
            EclssSimulation::new(),
            SabatierSimulation::new(),
            PowerSimulation::new(),
            ThermalSimulation::new(),
            WaterSimulation::new(),
            SafetySimulation::new(),
            default_runtime,
            default_runtime,
            default_runtime,
            default_runtime,
            default_runtime,
            default_runtime,
            default_runtime,
            SubsystemOperatorState::default(),
            SubsystemOperatorState::default(),
            SubsystemOperatorState::default(),
            SubsystemOperatorState::default(),
            SubsystemOperatorState::default(),
            SubsystemOperatorState::default(),
            PlantScheduler::new(plant_runtime_config),
            PlantRecoveryStatus {
                restored_from_checkpoint: false,
                downtime_policy: plant_runtime_config.downtime_policy,
                checkpoint_age_sec: 0.0,
                queued_catchup_sec: 0.0,
                journal_tail_records: 0,
                unapplied_state_records: 0,
            },
        ),
    };
    for (runtime, operator_state) in [
        (&mut eclss_runtime_state, &eclss_operator_state_value),
        (&mut sabatier_runtime_state, &sabatier_operator_state_value),
        (&mut power_runtime_state, &power_operator_state_value),
        (&mut thermal_runtime_state, &thermal_operator_state_value),
        (&mut water_runtime_state, &water_operator_state_value),
        (&mut safety_runtime_state, &safety_operator_state_value),
    ] {
        reconcile_runtime_with_operator(runtime, operator_state, initial_transition_ms);
    }
    let sim = Arc::new(RwLock::new(airlock_state));
    let airlock_runtime = Arc::new(RwLock::new(airlock_runtime_state));
    let eclss_runtime = Arc::new(RwLock::new(eclss_runtime_state));
    let sabatier_runtime = Arc::new(RwLock::new(sabatier_runtime_state));
    let power_runtime = Arc::new(RwLock::new(power_runtime_state));
    let thermal_runtime = Arc::new(RwLock::new(thermal_runtime_state));
    let water_runtime = Arc::new(RwLock::new(water_runtime_state));
    let safety_runtime = Arc::new(RwLock::new(safety_runtime_state));
    let eclss_operator_state = Arc::new(RwLock::new(eclss_operator_state_value));
    let sabatier_operator_state = Arc::new(RwLock::new(sabatier_operator_state_value));
    let power_operator_state = Arc::new(RwLock::new(power_operator_state_value));
    let thermal_operator_state = Arc::new(RwLock::new(thermal_operator_state_value));
    let water_operator_state = Arc::new(RwLock::new(water_operator_state_value));
    let safety_operator_state = Arc::new(RwLock::new(safety_operator_state_value));
    let eclss_sim = Arc::new(RwLock::new(eclss_state));
    let sabatier_sim = Arc::new(RwLock::new(sabatier_state));
    let power_sim = Arc::new(RwLock::new(power_state));
    let thermal_sim = Arc::new(RwLock::new(thermal_state));
    let water_sim = Arc::new(RwLock::new(water_state));
    let safety_sim = Arc::new(RwLock::new(safety_state));
    let tag_catalog = Arc::new(TagCatalog::full_base());
    tag_catalog
        .validate()
        .map_err(|error| anyhow::anyhow!("invalid canonical telemetry catalog: {error}"))?;
    info!(
        "Canonical telemetry catalog ready: {} tags, {:.1} nominal publications/s",
        tag_catalog.stats().canonical_tags,
        tag_catalog.stats().nominal_publications_per_second
    );
    let pea_opcua_endpoints = Arc::new(HashMap::from([
        (
            DEFAULT_AIRLOCK_PEA_ID.to_string(),
            opcua_endpoint_url.clone(),
        ),
        (DEFAULT_ECLSS_PEA_ID.to_string(), eclss_endpoint_url),
        (DEFAULT_SABATIER_PEA_ID.to_string(), sabatier_endpoint_url),
        (DEFAULT_POWER_PEA_ID.to_string(), power_endpoint_url),
        (DEFAULT_THERMAL_PEA_ID.to_string(), thermal_endpoint_url),
        (DEFAULT_WATER_PEA_ID.to_string(), water_endpoint_url),
        (DEFAULT_SAFETY_PEA_ID.to_string(), safety_endpoint_url),
    ]));
    let (snapshots_tx, _snapshots_rx) = broadcast::channel(256);
    let (systems_snapshots_tx, _systems_snapshots_rx) = broadcast::channel(256);
    let plant_runtime = Arc::new(RwLock::new(plant_scheduler.snapshot()));
    plant_persistence.append_journal(
        plant_scheduler.snapshot().plant_elapsed_sec,
        if plant_recovery.restored_from_checkpoint {
            "runtime_restored"
        } else {
            "plant_genesis"
        },
        plant_persistence.plant_id(),
        serde_json::to_value(plant_recovery)?,
    )?;

    let plant_transaction = Arc::new(Mutex::new(()));
    let opcua_control = opcua::spawn_opcua_server(
        sim.clone(),
        snapshots_tx.clone(),
        opcua_runtime_config,
        plant_transaction.clone(),
        plant_persistence.clone(),
        plant_runtime.clone(),
    );
    opcua_subsystems::spawn_eclss_opcua_server(
        eclss_sim.clone(),
        eclss_runtime.clone(),
        eclss_port,
        initial_security.clone(),
    );
    opcua_subsystems::spawn_sabatier_opcua_server(
        sabatier_sim.clone(),
        sabatier_runtime.clone(),
        sabatier_port,
        initial_security.clone(),
    );
    opcua_subsystems::spawn_power_opcua_server(
        power_sim.clone(),
        power_runtime.clone(),
        power_port,
        initial_security.clone(),
    );
    opcua_subsystems::spawn_thermal_opcua_server(
        thermal_sim.clone(),
        thermal_runtime.clone(),
        thermal_port,
        initial_security.clone(),
    );
    opcua_subsystems::spawn_water_opcua_server(
        water_sim.clone(),
        water_runtime.clone(),
        water_port,
        initial_security.clone(),
    );
    opcua_subsystems::spawn_safety_opcua_server(
        safety_sim.clone(),
        safety_runtime.clone(),
        safety_port,
        initial_security.clone(),
    );
    let context = AppContext {
        sim,
        airlock_runtime,
        eclss_runtime,
        sabatier_runtime,
        power_runtime,
        thermal_runtime,
        water_runtime,
        safety_runtime,
        eclss_operator_state,
        sabatier_operator_state,
        power_operator_state,
        thermal_operator_state,
        water_operator_state,
        safety_operator_state,
        eclss_sim,
        sabatier_sim,
        power_sim,
        thermal_sim,
        water_sim,
        safety_sim,
        pea_opcua_endpoints,
        zenoh_session,
        mqtt_uns,
        node_id,
        snapshots_tx,
        systems_snapshots_tx,
        plant_runtime,
        plant_persistence,
        plant_recovery,
        plant_transaction,
        opcua_control,
        next_client_id: Arc::new(AtomicU64::new(1)),
        tag_catalog,
        historian,
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let simulation_task = spawn_simulation_task(context.clone(), plant_scheduler, shutdown_rx);

    let frontend_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend");
    let index_file = frontend_dir.join("index.html");

    let app = Router::new()
        .route("/api/health", get(api_health))
        .route("/api/snapshot", get(api_snapshot))
        .route("/api/v1/power/snapshot", get(api_power_snapshot))
        .route("/api/v1/thermal/snapshot", get(api_thermal_snapshot))
        .route("/api/v1/water/snapshot", get(api_water_snapshot))
        .route("/api/v1/safety/snapshot", get(api_safety_snapshot))
        .route("/api/v1/safety/hazards", post(api_set_safety_hazards))
        .route("/api/v1/telemetry/catalog", get(api_telemetry_catalog))
        .route("/api/v1/telemetry/stats", get(api_telemetry_stats))
        .route("/api/v1/telemetry/history", get(api_telemetry_history))
        .route("/api/events", get(api_events))
        .route("/api/mtp/tree", get(api_mtp_tree))
        .route("/api/v1/pea", get(api_v1_list_peas))
        .route("/api/v1/pea/{pea_id}", get(api_v1_get_pea))
        .route("/api/v1/pea/{pea_id}/deploy", post(api_v1_deploy_pea))
        .route("/api/v1/pea/{pea_id}/start", post(api_v1_start_pea))
        .route("/api/v1/pea/{pea_id}/stop", post(api_v1_stop_pea))
        .route("/api/v1/pea/{pea_id}/undeploy", post(api_v1_undeploy_pea))
        .route("/api/v1/pea/{pea_id}/opcua", get(api_v1_get_pea_opcua))
        .route(
            "/api/v1/pea/{pea_id}/mtp/tree",
            get(api_v1_get_pea_mtp_tree),
        )
        .route(
            "/api/v1/pea/{pea_id}/operator-state",
            get(api_v1_get_subsystem_operator_state).post(api_v1_set_subsystem_operator_state),
        )
        .route("/api/v1/i3x/pea", get(api_v1_i3x_list_peas))
        .route("/api/v1/i3x/pea/{pea_id}", get(api_v1_i3x_get_pea))
        .route(
            "/api/v1/i3x/capability-schema",
            get(api_v1_i3x_capability_schema),
        )
        .route("/api/v1/namespaces", get(api_v1_i3x_namespaces))
        .route("/api/v1/objecttypes", get(api_v1_i3x_objecttypes))
        .route(
            "/api/v1/objecttypes/{element_id}",
            get(api_v1_i3x_objecttype_by_id),
        )
        .route(
            "/api/v1/relationshiptypes",
            get(api_v1_i3x_relationshiptypes),
        )
        .route(
            "/api/v1/relationshiptypes/{element_id}",
            get(api_v1_i3x_relationshiptype_by_id),
        )
        .route("/api/v1/objects", get(api_v1_i3x_objects))
        .route("/api/v1/objects/{element_id}", get(api_v1_i3x_object_by_id))
        .route(
            "/api/v1/objects/{element_id}/related",
            get(api_v1_i3x_related_objects),
        )
        .route(
            "/api/v1/objects/{element_id}/value",
            get(api_v1_i3x_object_value).put(api_v1_i3x_put_object_value),
        )
        .route(
            "/api/v1/objects/{element_id}/history",
            get(api_v1_i3x_object_history),
        )
        .route(
            "/api/v1/pea/{pea_id}/services/{service_tag}/command",
            post(api_v1_pea_service_command),
        )
        // MTP Compliance: Phase 2 API endpoints (Parameter Type System & Procedure Calling Convention)
        .route("/api/v2/pea/{pea_id}/manifest", get(api_v2_get_manifest))
        .route(
            "/api/v2/pea/{pea_id}/services/{service_name}",
            get(api_v2_get_service),
        )
        .route(
            "/api/v2/pea/{pea_id}/services/{service_name}/parameters",
            get(api_v2_list_service_parameters),
        )
        .route(
            "/api/v2/pea/{pea_id}/services/{service_name}/parameters/{param_name}",
            get(api_v2_get_parameter).post(api_v2_set_parameter),
        )
        .route(
            "/api/v2/pea/{pea_id}/services/{service_name}/procedures",
            get(api_v2_list_procedures),
        )
        .route(
            "/api/v2/pea/{pea_id}/services/{service_name}/procedures/{proc_name}",
            get(api_v2_get_procedure),
        )
        .route(
            "/api/v2/pea/{pea_id}/services/{service_name}/procedures/{proc_name}/request",
            post(api_v2_request_procedure),
        )
        .route(
            "/api/v2/pea/{pea_id}/services/{service_name}/procedures/{proc_name}/request/{request_id}",
            get(api_v2_get_procedure_status),
        )
        .route("/api/security/profile", post(api_set_security_profile))
        .route("/api/permissions", post(api_set_permissions))
        .route("/api/modes", post(api_set_modes))
        .route("/api/faults/leak-rate", post(api_set_leak_rate))
        .route("/api/faults/valve", post(api_set_valve_fault))
        .route("/api/commands/{source}/write", post(api_write_command))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new(frontend_dir).not_found_service(ServeFile::new(index_file)))
        .layer(TraceLayer::new_for_http())
        .with_state(context);

    let bind_addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(bind_addr).await?;
    info!("Mars airlock backend running on http://{}", bind_addr);
    let signal_shutdown_tx = shutdown_tx.clone();
    let server_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(signal_shutdown_tx))
    .await;
    let _ = shutdown_tx.send(true);
    simulation_task
        .await
        .map_err(|err| anyhow::anyhow!("plant scheduler task failed: {err}"))??;
    server_result?;

    Ok(())
}

fn is_state_journal_kind(kind: &str) -> bool {
    matches!(
        kind,
        "pea_lifecycle"
            | "operator_state_changed"
            | "service_command_processed"
            | "security_profile_changed"
            | "permissions_changed"
            | "operating_mode_changed"
            | "fault_injected"
            | "valve_fault_changed"
            | "command_processed"
            | "opcua_command_processed"
            | "procedure_requested"
    )
}

fn spawn_simulation_task(
    context: AppContext,
    mut scheduler: PlantScheduler,
    mut shutdown_rx: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        let mut ticker = time::interval(scheduler.wall_tick_duration());
        let mut publish_divider: u64 = 0;
        let mut uns_divider: u64 = 0;
        let checkpoint_interval_sec = context.plant_persistence.checkpoint_interval_sec();
        let mut next_checkpoint_sec = next_checkpoint_boundary(
            scheduler.snapshot().plant_elapsed_sec,
            checkpoint_interval_sec,
        );

        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        save_plant_checkpoint(
                            &context,
                            scheduler.state(),
                            "shutdown_checkpoint_saved",
                        )
                        .await?;
                        info!(
                            "Saved final plant checkpoint at simulated second {}",
                            scheduler.snapshot().plant_elapsed_sec
                        );
                        return Ok(());
                    }
                    continue;
                }
            }
            let steps_due = scheduler.begin_wall_tick();
            if steps_due == 0 {
                *context.plant_runtime.write().await = scheduler.snapshot();
                continue;
            }

            publish_divider = publish_divider.wrapping_add(1);
            uns_divider = uns_divider.wrapping_add(1);

            let eclss_running = {
                let runtime = context.eclss_runtime.read().await;
                runtime.deployed && runtime.running
            };
            let sabatier_running = {
                let runtime = context.sabatier_runtime.read().await;
                runtime.deployed && runtime.running
            };
            let power_running = {
                let runtime = context.power_runtime.read().await;
                runtime.deployed && runtime.running
            };
            let thermal_running = {
                let runtime = context.thermal_runtime.read().await;
                runtime.deployed && runtime.running
            };
            let water_running = {
                let runtime = context.water_runtime.read().await;
                runtime.deployed && runtime.running
            };
            let safety_running = {
                let runtime = context.safety_runtime.read().await;
                runtime.deployed && runtime.running
            };

            let mut eclss_snapshot = None;
            let mut sabatier_snapshot = None;
            let mut power_snapshot = None;
            let mut thermal_snapshot = None;
            let mut water_snapshot = None;
            let mut safety_snapshot = None;
            let mut historian_frames = Vec::new();
            for _ in 0..steps_due {
                let _transaction = context.plant_transaction.lock().await;
                let scheduled = scheduler.advance_fixed_step();
                let prior_power = context.power_sim.read().await.snapshot();
                let critical_power_available =
                    prior_power.bus_voltage_v >= 360.0 && prior_power.unmet_load_kw < 0.1;
                let flexible_power_available =
                    critical_power_available && !prior_power.load_shed_active;
                let latest_airlock = {
                    let mut sim = context.sim.write().await;
                    sim.step(scheduled.fixed_step_sec);
                    sim.snapshot()
                };

                let latest_eclss = {
                    let mut sim = context.eclss_sim.write().await;
                    sim.step(
                        scheduled.fixed_step_sec,
                        eclss_running && critical_power_available,
                    )
                };
                let latest_sabatier = {
                    let mut sim = context.sabatier_sim.write().await;
                    sim.step(
                        scheduled.fixed_step_sec,
                        sabatier_running && flexible_power_available,
                        latest_eclss.co2_capture_kgph,
                    )
                };
                let eclss_condensate_kgph = 0.25 + latest_eclss.water_recovery_pct / 100.0 * 0.35;
                let latest_water = {
                    let mut sim = context.water_sim.write().await;
                    sim.step(
                        scheduled.fixed_step_sec,
                        water_running,
                        critical_power_available,
                        latest_sabatier.water_production_kgph,
                        eclss_condensate_kgph,
                    )
                };
                let latest_safety = {
                    let mut sim = context.safety_sim.write().await;
                    sim.step(
                        scheduled.fixed_step_sec,
                        safety_running,
                        critical_power_available,
                        eclss_running && critical_power_available,
                    )
                };
                let airlock_load_kw = if latest_airlock.pump_on {
                    latest_airlock.pump_current_a * 400.0 / 1_000.0
                } else {
                    0.35
                };
                let latest_thermal = {
                    let mut sim = context.thermal_sim.write().await;
                    sim.step(
                        scheduled.fixed_step_sec,
                        thermal_running,
                        critical_power_available,
                        latest_eclss.power_kw,
                        latest_sabatier.power_kw,
                        latest_safety.fire_heat_release_kw,
                        prior_power.served_load_kw,
                    )
                };
                let latest_power = {
                    let mut sim = context.power_sim.write().await;
                    sim.step(
                        scheduled.fixed_step_sec,
                        power_running,
                        latest_eclss.power_kw,
                        latest_thermal.pump_electric_power_kw + latest_thermal.heater_power_kw,
                        latest_water.treatment_power_kw,
                        latest_safety.safety_power_kw,
                        latest_sabatier.power_kw,
                        airlock_load_kw,
                    )
                };
                if scheduled.run_medium {
                    historian_frames.push((
                        scheduler.snapshot().plant_elapsed_sec,
                        latest_eclss.clone(),
                        latest_power.clone(),
                        latest_water.clone(),
                        latest_safety.clone(),
                    ));
                }
                eclss_snapshot = Some(latest_eclss);
                sabatier_snapshot = Some(latest_sabatier);
                power_snapshot = Some(latest_power);
                thermal_snapshot = Some(latest_thermal);
                water_snapshot = Some(latest_water);
                safety_snapshot = Some(latest_safety);

                // These deterministic boundaries are hooks for the forthcoming
                // power/thermal and inventory/degradation model tiers.
                let _cadence_boundary = (scheduled.run_medium, scheduled.run_slow);
            }

            *context.plant_runtime.write().await = scheduler.snapshot();
            if scheduler.snapshot().plant_elapsed_sec >= next_checkpoint_sec {
                if let Err(err) =
                    save_plant_checkpoint(&context, scheduler.state(), "checkpoint_saved").await
                {
                    error!("Failed to save plant checkpoint: {err:#}");
                }
                next_checkpoint_sec = next_checkpoint_boundary(
                    scheduler.snapshot().plant_elapsed_sec,
                    checkpoint_interval_sec,
                );
            }
            let eclss_snapshot = eclss_snapshot.expect("at least one fixed step was scheduled");
            let sabatier_snapshot =
                sabatier_snapshot.expect("at least one fixed step was scheduled");
            let power_snapshot = power_snapshot.expect("at least one fixed step was scheduled");
            let thermal_snapshot = thermal_snapshot.expect("at least one fixed step was scheduled");
            let water_snapshot = water_snapshot.expect("at least one fixed step was scheduled");
            let safety_snapshot = safety_snapshot.expect("at least one fixed step was scheduled");
            if !historian_frames.is_empty()
                && let Err(err) = persist_core_historian_frames(&context, historian_frames).await
            {
                error!("Failed to append historian samples: {err:#}");
            }
            let maybe_airlock_snapshot = if publish_divider.is_multiple_of(2) {
                let sim = context.sim.read().await;
                Some(sim.snapshot())
            } else {
                None
            };

            // Construct combined systems snapshot for 3D visualization frontend
            if let Some(ref airlock_snap) = maybe_airlock_snapshot {
                let systems_snap = SystemsSnapshot {
                    timestamp_ms: Simulation::now_ms(),
                    // Airlock
                    chamber_pressure: airlock_snap.pressure_pa / 1000.0, // Convert Pa to kPa
                    inner_door_open: airlock_snap.inner_door_position_pct > 50.0,
                    outer_door_open: airlock_snap.outer_door_position_pct > 50.0,
                    // ECLSS
                    eclss_pressure: eclss_snapshot.cabin_pressure_kpa / 101.325, // Convert kPa to atm
                    eclss_status: if eclss_running {
                        "Operational".to_string()
                    } else {
                        "Standby".to_string()
                    },
                    co2_level: eclss_snapshot.co2_ppm,
                    o2_level: eclss_snapshot.o2_percent,
                    thermal_load: eclss_snapshot.power_kw,
                    humidity: eclss_snapshot.humidity_pct,
                    // Sabatier
                    reactor_temp: sabatier_snapshot.reactor_temp_c,
                    reactor_pressure: sabatier_snapshot.reactor_pressure_bar * 0.1, // bar to MPa
                    h2_flow: sabatier_snapshot.h2_feed_kgph,
                    co2_flow: sabatier_snapshot.co2_feed_kgph,
                    product_flow: sabatier_snapshot.methane_production_kgph
                        + sabatier_snapshot.water_production_kgph,
                    // Power
                    power_generation_kw: power_snapshot.generation_kw,
                    power_load_kw: power_snapshot.served_load_kw,
                    battery_soc_pct: power_snapshot.battery_soc_pct,
                    dc_bus_voltage_v: power_snapshot.bus_voltage_v,
                    power_load_shed_active: power_snapshot.load_shed_active,
                    habitat_temp_c: thermal_snapshot.habitat_temp_c,
                    thermal_heat_rejection_kw: thermal_snapshot.heat_rejection_kw,
                    thermal_cooling_available: thermal_snapshot.cooling_available,
                    potable_water_kg: water_snapshot.potable_water_kg,
                    wastewater_kg: water_snapshot.wastewater_kg,
                    water_quality_alarm: water_snapshot.alarm_water_quality,
                    habitat_pressure_kpa: safety_snapshot.habitat_pressure_kpa,
                    internal_radiation_msv_h: safety_snapshot.internal_radiation_msv_h,
                    safety_alarm_active: safety_alarm_active(&safety_snapshot),
                    // Status
                    healthy: !airlock_snap.alarms.high_pressure_alarm_active
                        && !airlock_snap.alarms.low_pressure_alarm_active
                        && !airlock_snap.alarms.leak_detected
                        && !thermal_snapshot.alarm_habitat_hot
                        && !thermal_snapshot.alarm_habitat_cold
                        && !thermal_snapshot.alarm_coolant_hot
                        && !water_snapshot.alarm_potable_low
                        && !water_snapshot.alarm_wastewater_high
                        && !water_snapshot.alarm_brine_high
                        && !water_snapshot.alarm_water_quality
                        && !safety_alarm_active(&safety_snapshot),
                };
                let _ = context.systems_snapshots_tx.send(systems_snap);
            }

            if let Some(snapshot) = maybe_airlock_snapshot {
                let _ = context.snapshots_tx.send(snapshot.clone());
                if uns_divider.is_multiple_of(10) {
                    let runtime_state = *context.airlock_runtime.read().await;
                    publish_pea_uns(&context, &snapshot, runtime_state).await;
                }
            }
            if uns_divider.is_multiple_of(10) {
                let eclss_runtime = *context.eclss_runtime.read().await;
                let eclss_operator_state = context.eclss_operator_state.read().await.clone();
                publish_subsystem_uns(
                    &context,
                    DEFAULT_ECLSS_PEA_ID,
                    ECLSS_SERVICE_TAG,
                    subsystem_service_state(eclss_runtime, &eclss_operator_state),
                    eclss_runtime,
                    eclss_snapshot.timestamp_ms,
                    json!({
                        "co2_ppm": eclss_snapshot.co2_ppm,
                        "o2_percent": eclss_snapshot.o2_percent,
                        "co2_capture_kgph": eclss_snapshot.co2_capture_kgph,
                        "o2_generation_kgph": eclss_snapshot.o2_generation_kgph,
                        "humidity_pct": eclss_snapshot.humidity_pct,
                        "water_recovery_pct": eclss_snapshot.water_recovery_pct,
                        "power_kw": eclss_snapshot.power_kw
                    }),
                )
                .await;

                let safety_runtime = *context.safety_runtime.read().await;
                let safety_operator_state = context.safety_operator_state.read().await.clone();
                publish_subsystem_uns(
                    &context,
                    DEFAULT_SAFETY_PEA_ID,
                    SAFETY_SERVICE_TAG,
                    subsystem_service_state(safety_runtime, &safety_operator_state),
                    safety_runtime,
                    safety_snapshot.timestamp_ms,
                    serde_json::to_value(&safety_snapshot).unwrap_or_else(|_| json!({})),
                )
                .await;

                let sabatier_runtime = *context.sabatier_runtime.read().await;
                let sabatier_operator_state = context.sabatier_operator_state.read().await.clone();
                publish_subsystem_uns(
                    &context,
                    DEFAULT_SABATIER_PEA_ID,
                    SABATIER_SERVICE_TAG,
                    subsystem_service_state(sabatier_runtime, &sabatier_operator_state),
                    sabatier_runtime,
                    sabatier_snapshot.timestamp_ms,
                    json!({
                        "reactor_temp_c": sabatier_snapshot.reactor_temp_c,
                        "reactor_pressure_bar": sabatier_snapshot.reactor_pressure_bar,
                        "co2_feed_kgph": sabatier_snapshot.co2_feed_kgph,
                        "h2_feed_kgph": sabatier_snapshot.h2_feed_kgph,
                        "conversion_efficiency_pct": sabatier_snapshot.conversion_efficiency_pct,
                        "methane_production_kgph": sabatier_snapshot.methane_production_kgph,
                        "water_production_kgph": sabatier_snapshot.water_production_kgph,
                        "power_kw": sabatier_snapshot.power_kw
                    }),
                )
                .await;

                let power_runtime = *context.power_runtime.read().await;
                let power_operator_state = context.power_operator_state.read().await.clone();
                publish_subsystem_uns(
                    &context,
                    DEFAULT_POWER_PEA_ID,
                    POWER_SERVICE_TAG,
                    subsystem_service_state(power_runtime, &power_operator_state),
                    power_runtime,
                    power_snapshot.timestamp_ms,
                    serde_json::to_value(&power_snapshot).unwrap_or_else(|_| json!({})),
                )
                .await;

                let thermal_runtime = *context.thermal_runtime.read().await;
                let thermal_operator_state = context.thermal_operator_state.read().await.clone();
                publish_subsystem_uns(
                    &context,
                    DEFAULT_THERMAL_PEA_ID,
                    THERMAL_SERVICE_TAG,
                    subsystem_service_state(thermal_runtime, &thermal_operator_state),
                    thermal_runtime,
                    thermal_snapshot.timestamp_ms,
                    serde_json::to_value(&thermal_snapshot).unwrap_or_else(|_| json!({})),
                )
                .await;

                let water_runtime = *context.water_runtime.read().await;
                let water_operator_state = context.water_operator_state.read().await.clone();
                publish_subsystem_uns(
                    &context,
                    DEFAULT_WATER_PEA_ID,
                    WATER_SERVICE_TAG,
                    subsystem_service_state(water_runtime, &water_operator_state),
                    water_runtime,
                    water_snapshot.timestamp_ms,
                    serde_json::to_value(&water_snapshot).unwrap_or_else(|_| json!({})),
                )
                .await;
            }
        }
    })
}

async fn persist_core_historian_frames(
    context: &AppContext,
    frames: Vec<(
        f64,
        EclssSnapshot,
        PowerSnapshot,
        WaterSnapshot,
        SafetySnapshot,
    )>,
) -> anyhow::Result<()> {
    let frames = frames
        .into_iter()
        .map(|(plant_elapsed_sec, eclss, power, water, safety)| {
            (
                plant_elapsed_sec,
                build_core_historian_samples(&eclss, &power, &water, &safety),
            )
        })
        .collect();
    let historian = context.historian.clone();
    tokio::task::spawn_blocking(move || historian.append_frames(wall_time_ms(), frames))
        .await
        .map_err(|error| anyhow::anyhow!("historian task failed: {error}"))??;
    Ok(())
}

fn build_core_historian_samples(
    eclss: &EclssSnapshot,
    power: &PowerSnapshot,
    water: &WaterSnapshot,
    safety: &SafetySnapshot,
) -> Vec<NewHistorianSample> {
    let eclss_quality = if eclss.alarm_high_co2 || eclss.alarm_low_o2 {
        TelemetryQuality::Uncertain
    } else if power.alarm_bus_undervoltage {
        TelemetryQuality::Bad
    } else {
        TelemetryQuality::Good
    };
    let power_quality = if power.alarm_bus_undervoltage {
        TelemetryQuality::Bad
    } else if power.alarm_battery_low || power.load_shed_active {
        TelemetryQuality::Uncertain
    } else {
        TelemetryQuality::Good
    };
    let water_alarm = water.alarm_potable_low
        || water.alarm_wastewater_high
        || water.alarm_brine_high
        || water.alarm_water_quality;
    let water_quality = if water.alarm_water_quality || water.unmet_crew_water_kgph > 0.0 {
        TelemetryQuality::Bad
    } else if water_alarm || !water.treatment_available {
        TelemetryQuality::Uncertain
    } else {
        TelemetryQuality::Good
    };
    let water_quality_code = match water_quality {
        TelemetryQuality::Good => 0,
        TelemetryQuality::Uncertain | TelemetryQuality::Stale => 1,
        TelemetryQuality::Bad => 2,
    };
    let safety_alarm = safety_alarm_active(safety);
    let safety_quality = if safety.alarm_low_pressure
        || safety.alarm_rapid_decompression
        || safety.alarm_fire
        || safety.alarm_toxic_gas
        || safety.alarm_structural
    {
        TelemetryQuality::Bad
    } else if safety.alarm_radiation || !safety.monitoring_available {
        TelemetryQuality::Uncertain
    } else {
        TelemetryQuality::Good
    };
    let source = "continuous_model_v1".to_string();
    vec![
        NewHistorianSample {
            tag_id: "underhill.v1.eclss.00000.pressure".to_string(),
            value: json!(eclss.cabin_pressure_kpa),
            quality: eclss_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.eclss.00000.oxygen".to_string(),
            value: json!(eclss.o2_percent),
            quality: eclss_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.eclss.00000.carbon_dioxide".to_string(),
            value: json!(eclss.co2_ppm),
            quality: eclss_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.eclss.00000.humidity".to_string(),
            value: json!(eclss.humidity_pct),
            quality: eclss_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.eclss.00000.alarm_active".to_string(),
            value: json!(eclss.alarm_high_co2 || eclss.alarm_low_o2),
            quality: eclss_quality,
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.power.00000.state_of_charge".to_string(),
            value: json!(power.battery_soc_pct),
            quality: power_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.power.00000.protection_state".to_string(),
            value: json!(if power.alarm_bus_undervoltage {
                "bus_undervoltage"
            } else if power.load_shed_active {
                "load_shed"
            } else if power.alarm_battery_low {
                "battery_low"
            } else {
                "normal"
            }),
            quality: power_quality,
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.water_waste.00000.true_value".to_string(),
            value: json!((water.potable_water_kg / water.potable_capacity_kg).clamp(0.0, 1.0)),
            quality: water_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.water_waste.00000.residual".to_string(),
            value: json!(water.instantaneous_balance_error_kgph.clamp(-1.0, 1.0)),
            quality: water_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.water_waste.00000.alarm_active".to_string(),
            value: json!(water_alarm),
            quality: water_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.water_waste.00000.quality_code".to_string(),
            value: json!(water_quality_code),
            quality: water_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.water_waste.00000.health_state".to_string(),
            value: json!(if matches!(water_quality, TelemetryQuality::Bad) {
                "fault"
            } else if matches!(
                water_quality,
                TelemetryQuality::Uncertain | TelemetryQuality::Stale
            ) {
                "degraded"
            } else {
                "normal"
            }),
            quality: water_quality,
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.safety_structure.00000.pressure".to_string(),
            value: json!(safety.habitat_pressure_kpa),
            quality: safety_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.safety_structure.00000.sensor_residual".to_string(),
            value: json!(
                safety
                    .instantaneous_mass_balance_error_kgph
                    .clamp(-10.0, 10.0)
            ),
            quality: safety_quality.clone(),
            source: source.clone(),
        },
        NewHistorianSample {
            tag_id: "underhill.v1.safety_structure.00000.alarm_active".to_string(),
            value: json!(safety_alarm),
            quality: safety_quality,
            source,
        },
    ]
}

fn next_checkpoint_boundary(elapsed_sec: f64, interval_sec: f64) -> f64 {
    ((elapsed_sec / interval_sec).floor() + 1.0) * interval_sec
}

async fn shutdown_signal(shutdown_tx: watch::Sender<bool>) {
    #[cfg(unix)]
    {
        let mut terminate =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(signal) => signal,
                Err(err) => {
                    error!("Failed to install SIGTERM handler: {err}");
                    let _ = tokio::signal::ctrl_c().await;
                    let _ = shutdown_tx.send(true);
                    return;
                }
            };
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(err) = result {
                    error!("Ctrl-C signal handler failed: {err}");
                }
            }
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    if let Err(err) = tokio::signal::ctrl_c().await {
        error!("Ctrl-C signal handler failed: {err}");
    }

    let _ = shutdown_tx.send(true);
}

async fn capture_plant_checkpoint(
    context: &AppContext,
    scheduler: PlantSchedulerState,
) -> anyhow::Result<PlantCheckpoint> {
    Ok(PlantCheckpoint::new(
        context.plant_persistence.plant_id().to_string(),
        context.plant_persistence.journal_sequence()?,
        scheduler,
        context.sim.read().await.clone(),
        context.eclss_sim.read().await.clone(),
        context.sabatier_sim.read().await.clone(),
        context.power_sim.read().await.clone(),
        context.thermal_sim.read().await.clone(),
        context.water_sim.read().await.clone(),
        context.safety_sim.read().await.clone(),
        *context.airlock_runtime.read().await,
        *context.eclss_runtime.read().await,
        *context.sabatier_runtime.read().await,
        *context.power_runtime.read().await,
        *context.thermal_runtime.read().await,
        *context.water_runtime.read().await,
        *context.safety_runtime.read().await,
        context.eclss_operator_state.read().await.clone(),
        context.sabatier_operator_state.read().await.clone(),
        context.power_operator_state.read().await.clone(),
        context.thermal_operator_state.read().await.clone(),
        context.water_operator_state.read().await.clone(),
        context.safety_operator_state.read().await.clone(),
    ))
}

async fn save_plant_checkpoint(
    context: &AppContext,
    scheduler: PlantSchedulerState,
    journal_kind: &'static str,
) -> anyhow::Result<()> {
    let _transaction = context.plant_transaction.lock().await;
    let checkpoint = capture_plant_checkpoint(context, scheduler).await?;
    let persistence = context.plant_persistence.clone();
    tokio::task::spawn_blocking(move || {
        persistence.save_checkpoint(&checkpoint)?;
        persistence.append_journal(
            checkpoint.scheduler.plant_elapsed_sec,
            journal_kind,
            persistence.plant_id(),
            json!({
                "step_index": checkpoint.scheduler.step_index,
                "saved_wall_time_ms": checkpoint.saved_wall_time_ms
            }),
        )?;
        anyhow::Ok(())
    })
    .await
    .map_err(|err| anyhow::anyhow!("checkpoint writer task failed: {err}"))??;
    Ok(())
}

async fn journal_operation(
    context: &AppContext,
    kind: &'static str,
    subject: String,
    payload: serde_json::Value,
) {
    let plant_elapsed_sec = context.plant_runtime.read().await.plant_elapsed_sec;
    let persistence = context.plant_persistence.clone();
    let result = tokio::task::spawn_blocking(move || {
        persistence.append_journal(plant_elapsed_sec, kind, subject, payload)
    })
    .await;
    match result {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => error!("Failed to append operational journal record: {err:#}"),
        Err(err) => error!("Operational journal task failed: {err}"),
    }
}

async fn api_health(State(context): State<AppContext>) -> impl IntoResponse {
    let plant_runtime = *context.plant_runtime.read().await;
    let journal_sequence = context
        .plant_persistence
        .journal_sequence()
        .unwrap_or_default();
    axum::Json(json!({
        "status": "ok",
        "service": "underhill-base-backend",
        "plant_runtime": plant_runtime,
        "plant_recovery": context.plant_recovery,
        "persistence": {
            "plant_id": context.plant_persistence.plant_id(),
            "checkpoint_interval_sec": context.plant_persistence.checkpoint_interval_sec(),
            "journal_sequence": journal_sequence
        },
        "telemetry_catalog": context.tag_catalog.stats(),
        "historian": context.historian.stats()
    }))
}

async fn api_telemetry_stats(State(context): State<AppContext>) -> impl IntoResponse {
    axum::Json(context.tag_catalog.stats().clone())
}

async fn api_telemetry_history(
    Query(query): Query<TelemetryHistoryQuery>,
    State(context): State<AppContext>,
) -> impl IntoResponse {
    let items = context.historian.query(
        query.tag_id.as_deref(),
        query.since_ms,
        query.limit.unwrap_or(500),
    );
    axum::Json(json!({
        "stats": context.historian.stats(),
        "count": items.len(),
        "items": items
    }))
}

async fn api_telemetry_catalog(
    Query(query): Query<TelemetryCatalogQuery>,
    State(context): State<AppContext>,
) -> impl IntoResponse {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(250).clamp(1, 5_000);
    let matches = |tag: &&CanonicalTag| {
        query
            .subsystem_family
            .as_ref()
            .is_none_or(|value| tag.subsystem_family.eq_ignore_ascii_case(value.trim()))
            && query
                .owner_pea
                .as_ref()
                .is_none_or(|value| tag.owner_pea.eq_ignore_ascii_case(value.trim()))
            && query
                .publication_class
                .as_ref()
                .is_none_or(|value| tag.publication_class.eq_ignore_ascii_case(value.trim()))
            && query
                .activation_state
                .as_ref()
                .is_none_or(|value| tag.activation_state.eq_ignore_ascii_case(value.trim()))
    };
    let total_matching = context.tag_catalog.tags().iter().filter(&matches).count();
    let items = context
        .tag_catalog
        .tags()
        .iter()
        .filter(matches)
        .skip(offset)
        .take(limit)
        .cloned()
        .collect();
    let next_offset = (offset + limit < total_matching).then_some(offset + limit);
    axum::Json(TelemetryCatalogPage {
        schema_version: context.tag_catalog.stats().schema_version,
        total_matching,
        offset,
        limit,
        next_offset,
        items,
    })
}

async fn api_snapshot(State(context): State<AppContext>) -> impl IntoResponse {
    let snapshot = {
        let sim = context.sim.read().await;
        sim.snapshot()
    };
    axum::Json(snapshot)
}

async fn api_power_snapshot(State(context): State<AppContext>) -> impl IntoResponse {
    axum::Json(context.power_sim.read().await.snapshot())
}

async fn api_thermal_snapshot(State(context): State<AppContext>) -> impl IntoResponse {
    axum::Json(context.thermal_sim.read().await.snapshot())
}

async fn api_water_snapshot(State(context): State<AppContext>) -> impl IntoResponse {
    axum::Json(context.water_sim.read().await.snapshot())
}

async fn api_safety_snapshot(State(context): State<AppContext>) -> impl IntoResponse {
    axum::Json(context.safety_sim.read().await.snapshot())
}

async fn api_set_safety_hazards(
    State(context): State<AppContext>,
    axum::Json(request): axum::Json<SafetyHazardUpdateRequest>,
) -> Result<axum::Json<SafetySnapshot>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    let snapshot = {
        let mut sim = context.safety_sim.write().await;
        sim.set_hazards(
            request.injected_leak_kg_s,
            request.fire_source_kw,
            request.habitat_isolated,
        )
        .map_err(|error| (StatusCode::BAD_REQUEST, error))?;
        sim.snapshot()
    };
    journal_operation(
        &context,
        "fault_injected",
        DEFAULT_SAFETY_PEA_ID.to_string(),
        json!({
            "injected_leak_kg_s": request.injected_leak_kg_s,
            "fire_source_kw": request.fire_source_kw,
            "habitat_isolated": request.habitat_isolated,
        }),
    )
    .await;
    Ok(axum::Json(snapshot))
}

async fn api_events(State(context): State<AppContext>) -> impl IntoResponse {
    let events: Vec<EventEntry> = {
        let sim = context.sim.read().await;
        sim.events()
    };
    axum::Json(events)
}

async fn api_mtp_tree(State(context): State<AppContext>) -> impl IntoResponse {
    let tree = {
        let sim = context.sim.read().await;
        sim.mtp_tree()
    };
    axum::Json(tree)
}

async fn api_v1_list_peas(State(context): State<AppContext>) -> impl IntoResponse {
    let (airlock_snapshot, airlock_runtime) = {
        let sim = context.sim.read().await;
        let runtime_state = *context.airlock_runtime.read().await;
        (sim.snapshot(), runtime_state)
    };
    let eclss_runtime = *context.eclss_runtime.read().await;
    let eclss_operator_state = context.eclss_operator_state.read().await.clone();
    let eclss_snapshot = context.eclss_sim.read().await.snapshot();
    let sabatier_runtime = *context.sabatier_runtime.read().await;
    let sabatier_operator_state = context.sabatier_operator_state.read().await.clone();
    let sabatier_snapshot = context.sabatier_sim.read().await.snapshot();
    let power_runtime = *context.power_runtime.read().await;
    let power_operator_state = context.power_operator_state.read().await.clone();
    let power_snapshot = context.power_sim.read().await.snapshot();
    let thermal_runtime = *context.thermal_runtime.read().await;
    let thermal_operator_state = context.thermal_operator_state.read().await.clone();
    let thermal_snapshot = context.thermal_sim.read().await.snapshot();
    let water_runtime = *context.water_runtime.read().await;
    let water_operator_state = context.water_operator_state.read().await.clone();
    let water_snapshot = context.water_sim.read().await.snapshot();
    let safety_runtime = *context.safety_runtime.read().await;
    let safety_operator_state = context.safety_operator_state.read().await.clone();
    let safety_snapshot = context.safety_sim.read().await.snapshot();
    let items = vec![
        build_airlock_pea_descriptor(&airlock_snapshot, airlock_runtime),
        build_eclss_pea_descriptor(
            &context,
            &eclss_snapshot,
            eclss_runtime,
            &eclss_operator_state,
            DEFAULT_ECLSS_PEA_ID,
            ECLSS_SERVICE_TAG,
        ),
        build_sabatier_pea_descriptor(
            &context,
            &sabatier_snapshot,
            sabatier_runtime,
            &sabatier_operator_state,
            DEFAULT_SABATIER_PEA_ID,
            SABATIER_SERVICE_TAG,
        ),
        build_power_pea_descriptor(
            &context,
            &power_snapshot,
            power_runtime,
            &power_operator_state,
        ),
        build_thermal_pea_descriptor(
            &context,
            &thermal_snapshot,
            thermal_runtime,
            &thermal_operator_state,
        ),
        build_water_pea_descriptor(
            &context,
            &water_snapshot,
            water_runtime,
            &water_operator_state,
        ),
        build_safety_pea_descriptor(
            &context,
            &safety_snapshot,
            safety_runtime,
            &safety_operator_state,
        ),
    ];
    axum::Json(json!({
        "items": items,
        "count": 7
    }))
}

async fn api_v1_get_pea(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    match pea_id.as_str() {
        DEFAULT_AIRLOCK_PEA_ID => {
            let (snapshot, runtime_state) = {
                let sim = context.sim.read().await;
                let runtime_state = *context.airlock_runtime.read().await;
                (sim.snapshot(), runtime_state)
            };
            Ok(axum::Json(build_airlock_pea_descriptor(
                &snapshot,
                runtime_state,
            )))
        }
        DEFAULT_ECLSS_PEA_ID => {
            let runtime_state = *context.eclss_runtime.read().await;
            let operator_state = context.eclss_operator_state.read().await.clone();
            let snapshot = context.eclss_sim.read().await.snapshot();
            Ok(axum::Json(build_eclss_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
                DEFAULT_ECLSS_PEA_ID,
                ECLSS_SERVICE_TAG,
            )))
        }
        DEFAULT_SABATIER_PEA_ID => {
            let runtime_state = *context.sabatier_runtime.read().await;
            let operator_state = context.sabatier_operator_state.read().await.clone();
            let snapshot = context.sabatier_sim.read().await.snapshot();
            Ok(axum::Json(build_sabatier_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
                DEFAULT_SABATIER_PEA_ID,
                SABATIER_SERVICE_TAG,
            )))
        }
        DEFAULT_POWER_PEA_ID => {
            let runtime_state = *context.power_runtime.read().await;
            let operator_state = context.power_operator_state.read().await.clone();
            let snapshot = context.power_sim.read().await.snapshot();
            Ok(axum::Json(build_power_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
            )))
        }
        DEFAULT_THERMAL_PEA_ID => {
            let runtime_state = *context.thermal_runtime.read().await;
            let operator_state = context.thermal_operator_state.read().await.clone();
            let snapshot = context.thermal_sim.read().await.snapshot();
            Ok(axum::Json(build_thermal_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
            )))
        }
        DEFAULT_WATER_PEA_ID => {
            let runtime_state = *context.water_runtime.read().await;
            let operator_state = context.water_operator_state.read().await.clone();
            let snapshot = context.water_sim.read().await.snapshot();
            Ok(axum::Json(build_water_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
            )))
        }
        DEFAULT_SAFETY_PEA_ID => {
            let runtime_state = *context.safety_runtime.read().await;
            let operator_state = context.safety_operator_state.read().await.clone();
            let snapshot = context.safety_sim.read().await.snapshot();
            Ok(axum::Json(build_safety_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
            )))
        }
        _ => Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}"))),
    }
}

async fn api_v1_get_pea_opcua(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let Some(endpoint_url) = context.pea_opcua_endpoints.get(&pea_id) else {
        return Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")));
    };

    if pea_id == DEFAULT_AIRLOCK_PEA_ID {
        let snapshot = {
            let sim = context.sim.read().await;
            sim.snapshot()
        };
        return Ok(axum::Json(json!({
            "pea_id": pea_id,
            "endpoint_url": endpoint_url,
            "active_security_mode": snapshot.diagnostics.active_security_mode,
            "security_modes_enabled": snapshot.diagnostics.security_modes_enabled,
            "namespace_uri": "urn:mars-airlock:mtp",
        })));
    }

    let namespace_uri = definition_for(&pea_id)
        .map(|definition| definition.namespace_uri)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")))?;
    Ok(axum::Json(json!({
        "pea_id": pea_id,
        "endpoint_url": endpoint_url,
        "active_security_mode": "NONE",
        "security_modes_enabled": ["NONE"],
        "namespace_uri": namespace_uri,
        "opcua_online": true
    })))
}

async fn api_v1_deploy_pea(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    let transition_ms = Simulation::now_ms();

    if pea_id == DEFAULT_AIRLOCK_PEA_ID {
        {
            let mut runtime = context.airlock_runtime.write().await;
            runtime.deployed = true;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }

        let snapshot = {
            let mut sim = context.sim.write().await;
            sim.set_modes(MtpModesUpdateRequest {
                operation_mode: Some(OperationMode::Off),
                command_en: Some(false),
                command_en_reason: Some("PEA deployed, not started".to_string()),
            });
            sim.snapshot()
        };
        let _ = context.snapshots_tx.send(snapshot.clone());
        let runtime_state = *context.airlock_runtime.read().await;
        publish_pea_uns(&context, &snapshot, runtime_state).await;
    } else if pea_id == DEFAULT_ECLSS_PEA_ID {
        {
            let mut runtime = context.eclss_runtime.write().await;
            runtime.deployed = true;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
        let runtime = *context.eclss_runtime.read().await;
        let operator_state = context.eclss_operator_state.read().await.clone();
        let snap = context.eclss_sim.read().await.snapshot();
        publish_subsystem_uns(
            &context,
            DEFAULT_ECLSS_PEA_ID,
            ECLSS_SERVICE_TAG,
            subsystem_service_state(runtime, &operator_state),
            runtime,
            snap.timestamp_ms,
            serde_json::to_value(&snap).unwrap_or_else(|_| json!({})),
        )
        .await;
    } else if pea_id == DEFAULT_SABATIER_PEA_ID {
        {
            let mut runtime = context.sabatier_runtime.write().await;
            runtime.deployed = true;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
        let runtime = *context.sabatier_runtime.read().await;
        let operator_state = context.sabatier_operator_state.read().await.clone();
        let snap = context.sabatier_sim.read().await.snapshot();
        publish_subsystem_uns(
            &context,
            DEFAULT_SABATIER_PEA_ID,
            SABATIER_SERVICE_TAG,
            subsystem_service_state(runtime, &operator_state),
            runtime,
            snap.timestamp_ms,
            serde_json::to_value(&snap).unwrap_or_else(|_| json!({})),
        )
        .await;
    } else if pea_id == DEFAULT_POWER_PEA_ID {
        {
            let mut runtime = context.power_runtime.write().await;
            runtime.deployed = true;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
        let runtime = *context.power_runtime.read().await;
        let operator_state = context.power_operator_state.read().await.clone();
        let snap = context.power_sim.read().await.snapshot();
        publish_subsystem_uns(
            &context,
            DEFAULT_POWER_PEA_ID,
            POWER_SERVICE_TAG,
            subsystem_service_state(runtime, &operator_state),
            runtime,
            snap.timestamp_ms,
            serde_json::to_value(&snap).unwrap_or_else(|_| json!({})),
        )
        .await;
    } else if pea_id == DEFAULT_THERMAL_PEA_ID {
        {
            let mut runtime = context.thermal_runtime.write().await;
            runtime.deployed = true;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
        let runtime = *context.thermal_runtime.read().await;
        let operator_state = context.thermal_operator_state.read().await.clone();
        let snap = context.thermal_sim.read().await.snapshot();
        publish_subsystem_uns(
            &context,
            DEFAULT_THERMAL_PEA_ID,
            THERMAL_SERVICE_TAG,
            subsystem_service_state(runtime, &operator_state),
            runtime,
            snap.timestamp_ms,
            serde_json::to_value(&snap).unwrap_or_else(|_| json!({})),
        )
        .await;
    } else if pea_id == DEFAULT_WATER_PEA_ID {
        {
            let mut runtime = context.water_runtime.write().await;
            runtime.deployed = true;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
        let runtime = *context.water_runtime.read().await;
        let operator_state = context.water_operator_state.read().await.clone();
        let snap = context.water_sim.read().await.snapshot();
        publish_subsystem_uns(
            &context,
            DEFAULT_WATER_PEA_ID,
            WATER_SERVICE_TAG,
            subsystem_service_state(runtime, &operator_state),
            runtime,
            snap.timestamp_ms,
            serde_json::to_value(&snap).unwrap_or_else(|_| json!({})),
        )
        .await;
    } else if pea_id == DEFAULT_SAFETY_PEA_ID {
        {
            let mut runtime = context.safety_runtime.write().await;
            runtime.deployed = true;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
        let runtime = *context.safety_runtime.read().await;
        let operator_state = context.safety_operator_state.read().await.clone();
        let snap = context.safety_sim.read().await.snapshot();
        publish_subsystem_uns(
            &context,
            DEFAULT_SAFETY_PEA_ID,
            SAFETY_SERVICE_TAG,
            subsystem_service_state(runtime, &operator_state),
            runtime,
            snap.timestamp_ms,
            serde_json::to_value(&snap).unwrap_or_else(|_| json!({})),
        )
        .await;
    } else {
        return Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")));
    }

    journal_operation(
        &context,
        "pea_lifecycle",
        pea_id.clone(),
        json!({ "transition": "deployed", "last_transition_ms": transition_ms }),
    )
    .await;

    Ok(axum::Json(json!({
        "pea_id": pea_id,
        "status": "deployed",
        "deployed": true,
        "running": false,
        "last_transition_ms": transition_ms
    })))
}

async fn api_v1_start_pea(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    let transition_ms = Simulation::now_ms();

    if pea_id == DEFAULT_AIRLOCK_PEA_ID {
        {
            let mut runtime = context.airlock_runtime.write().await;
            if !runtime.deployed {
                return Err((
                    StatusCode::CONFLICT,
                    format!("PEA {pea_id} is not deployed"),
                ));
            }
            runtime.running = true;
            runtime.last_transition_ms = transition_ms;
        }

        let snapshot = {
            let mut sim = context.sim.write().await;
            sim.set_modes(MtpModesUpdateRequest {
                operation_mode: Some(OperationMode::Auto),
                command_en: Some(true),
                command_en_reason: Some(String::new()),
            });
            sim.snapshot()
        };
        let _ = context.snapshots_tx.send(snapshot.clone());
        let runtime_state = *context.airlock_runtime.read().await;
        publish_pea_uns(&context, &snapshot, runtime_state).await;
    } else if pea_id == DEFAULT_ECLSS_PEA_ID {
        {
            ensure_subsystem_start_allowed(&pea_id, &*context.eclss_operator_state.read().await)?;
            let mut runtime = context.eclss_runtime.write().await;
            if !runtime.deployed {
                return Err((
                    StatusCode::CONFLICT,
                    format!("PEA {pea_id} is not deployed"),
                ));
            }
            runtime.running = true;
            runtime.last_transition_ms = transition_ms;
        }
    } else if pea_id == DEFAULT_SABATIER_PEA_ID {
        {
            ensure_subsystem_start_allowed(
                &pea_id,
                &*context.sabatier_operator_state.read().await,
            )?;
            let mut runtime = context.sabatier_runtime.write().await;
            if !runtime.deployed {
                return Err((
                    StatusCode::CONFLICT,
                    format!("PEA {pea_id} is not deployed"),
                ));
            }
            runtime.running = true;
            runtime.last_transition_ms = transition_ms;
        }
    } else if pea_id == DEFAULT_POWER_PEA_ID {
        ensure_subsystem_start_allowed(&pea_id, &*context.power_operator_state.read().await)?;
        let mut runtime = context.power_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = true;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_THERMAL_PEA_ID {
        ensure_subsystem_start_allowed(&pea_id, &*context.thermal_operator_state.read().await)?;
        let mut runtime = context.thermal_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = true;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_WATER_PEA_ID {
        ensure_subsystem_start_allowed(&pea_id, &*context.water_operator_state.read().await)?;
        let mut runtime = context.water_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = true;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_SAFETY_PEA_ID {
        ensure_subsystem_start_allowed(&pea_id, &*context.safety_operator_state.read().await)?;
        let mut runtime = context.safety_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = true;
        runtime.last_transition_ms = transition_ms;
    } else {
        return Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")));
    }

    journal_operation(
        &context,
        "pea_lifecycle",
        pea_id.clone(),
        json!({ "transition": "started", "last_transition_ms": transition_ms }),
    )
    .await;

    Ok(axum::Json(json!({
        "pea_id": pea_id,
        "status": "running",
        "deployed": true,
        "running": true,
        "last_transition_ms": transition_ms
    })))
}

async fn api_v1_stop_pea(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    let transition_ms = Simulation::now_ms();

    if pea_id == DEFAULT_AIRLOCK_PEA_ID {
        {
            let mut runtime = context.airlock_runtime.write().await;
            if !runtime.deployed {
                return Err((
                    StatusCode::CONFLICT,
                    format!("PEA {pea_id} is not deployed"),
                ));
            }
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }

        let snapshot = {
            let mut sim = context.sim.write().await;
            sim.set_modes(MtpModesUpdateRequest {
                operation_mode: Some(OperationMode::Off),
                command_en: Some(false),
                command_en_reason: Some("PEA stopped by lifecycle".to_string()),
            });
            sim.snapshot()
        };
        let _ = context.snapshots_tx.send(snapshot.clone());
        let runtime_state = *context.airlock_runtime.read().await;
        publish_pea_uns(&context, &snapshot, runtime_state).await;
    } else if pea_id == DEFAULT_ECLSS_PEA_ID {
        {
            let mut runtime = context.eclss_runtime.write().await;
            if !runtime.deployed {
                return Err((
                    StatusCode::CONFLICT,
                    format!("PEA {pea_id} is not deployed"),
                ));
            }
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
    } else if pea_id == DEFAULT_SABATIER_PEA_ID {
        {
            let mut runtime = context.sabatier_runtime.write().await;
            if !runtime.deployed {
                return Err((
                    StatusCode::CONFLICT,
                    format!("PEA {pea_id} is not deployed"),
                ));
            }
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }
    } else if pea_id == DEFAULT_POWER_PEA_ID {
        let mut runtime = context.power_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_THERMAL_PEA_ID {
        let mut runtime = context.thermal_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_WATER_PEA_ID {
        let mut runtime = context.water_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_SAFETY_PEA_ID {
        let mut runtime = context.safety_runtime.write().await;
        if !runtime.deployed {
            return Err((
                StatusCode::CONFLICT,
                format!("PEA {pea_id} is not deployed"),
            ));
        }
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else {
        return Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")));
    }

    journal_operation(
        &context,
        "pea_lifecycle",
        pea_id.clone(),
        json!({ "transition": "stopped", "last_transition_ms": transition_ms }),
    )
    .await;

    Ok(axum::Json(json!({
        "pea_id": pea_id,
        "status": "stopped",
        "deployed": true,
        "running": false,
        "last_transition_ms": transition_ms
    })))
}

async fn api_v1_undeploy_pea(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    let transition_ms = Simulation::now_ms();

    if pea_id == DEFAULT_AIRLOCK_PEA_ID {
        {
            let mut runtime = context.airlock_runtime.write().await;
            runtime.deployed = false;
            runtime.running = false;
            runtime.last_transition_ms = transition_ms;
        }

        let snapshot = {
            let mut sim = context.sim.write().await;
            sim.set_modes(MtpModesUpdateRequest {
                operation_mode: Some(OperationMode::Off),
                command_en: Some(false),
                command_en_reason: Some("PEA undeployed".to_string()),
            });
            sim.snapshot()
        };
        let _ = context.snapshots_tx.send(snapshot.clone());
        let runtime_state = *context.airlock_runtime.read().await;
        publish_pea_uns(&context, &snapshot, runtime_state).await;
    } else if pea_id == DEFAULT_ECLSS_PEA_ID {
        let mut runtime = context.eclss_runtime.write().await;
        runtime.deployed = false;
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_SABATIER_PEA_ID {
        let mut runtime = context.sabatier_runtime.write().await;
        runtime.deployed = false;
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_POWER_PEA_ID {
        let mut runtime = context.power_runtime.write().await;
        runtime.deployed = false;
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_THERMAL_PEA_ID {
        let mut runtime = context.thermal_runtime.write().await;
        runtime.deployed = false;
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_WATER_PEA_ID {
        let mut runtime = context.water_runtime.write().await;
        runtime.deployed = false;
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else if pea_id == DEFAULT_SAFETY_PEA_ID {
        let mut runtime = context.safety_runtime.write().await;
        runtime.deployed = false;
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    } else {
        return Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")));
    }

    journal_operation(
        &context,
        "pea_lifecycle",
        pea_id.clone(),
        json!({ "transition": "undeployed", "last_transition_ms": transition_ms }),
    )
    .await;

    Ok(axum::Json(json!({
        "pea_id": pea_id,
        "status": "undeployed",
        "deployed": false,
        "running": false,
        "last_transition_ms": transition_ms
    })))
}

async fn api_v1_get_pea_mtp_tree(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    if pea_id == DEFAULT_AIRLOCK_PEA_ID {
        let (snapshot, tree) = {
            let sim = context.sim.read().await;
            (sim.snapshot(), sim.mtp_tree())
        };
        return Ok(axum::Json(json!({
            "pea_id": snapshot.mtp_runtime.pea_information_label.tag_name,
            "namespace": tree.namespace,
            "root_path": tree.root_path,
            "nodes": tree.nodes
        })));
    }
    if pea_id == DEFAULT_ECLSS_PEA_ID {
        let nodes = context.eclss_sim.read().await.mtp_nodes();
        return Ok(axum::Json(json!({
            "pea_id": DEFAULT_ECLSS_PEA_ID,
            "namespace": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_ECLSS_PEA_ID),
            "root_path": "Objects/Underhill/ECLSSPEA",
            "nodes": nodes
        })));
    }
    if pea_id == DEFAULT_SABATIER_PEA_ID {
        let nodes = context.sabatier_sim.read().await.mtp_nodes();
        return Ok(axum::Json(json!({
            "pea_id": DEFAULT_SABATIER_PEA_ID,
            "namespace": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_SABATIER_PEA_ID),
            "root_path": "Objects/Underhill/SabatierPEA",
            "nodes": nodes
        })));
    }
    if pea_id == DEFAULT_POWER_PEA_ID {
        let nodes = context.power_sim.read().await.mtp_nodes();
        return Ok(axum::Json(json!({
            "pea_id": DEFAULT_POWER_PEA_ID,
            "namespace": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_POWER_PEA_ID),
            "root_path": "Objects/Underhill/PowerPEA",
            "nodes": nodes
        })));
    }
    if pea_id == DEFAULT_THERMAL_PEA_ID {
        let nodes = context.thermal_sim.read().await.mtp_nodes();
        return Ok(axum::Json(json!({
            "pea_id": DEFAULT_THERMAL_PEA_ID,
            "namespace": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_THERMAL_PEA_ID),
            "root_path": "Objects/Underhill/ThermalPEA",
            "nodes": nodes
        })));
    }
    if pea_id == DEFAULT_WATER_PEA_ID {
        let nodes = context.water_sim.read().await.mtp_nodes();
        return Ok(axum::Json(json!({
            "pea_id": DEFAULT_WATER_PEA_ID,
            "namespace": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_WATER_PEA_ID),
            "root_path": "Objects/Underhill/WaterPEA",
            "nodes": nodes
        })));
    }
    if pea_id == DEFAULT_SAFETY_PEA_ID {
        let nodes = context.safety_sim.read().await.mtp_nodes();
        return Ok(axum::Json(json!({
            "pea_id": DEFAULT_SAFETY_PEA_ID,
            "namespace": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_SAFETY_PEA_ID),
            "root_path": "Objects/Underhill/SafetyPEA",
            "nodes": nodes
        })));
    }
    Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")))
}

async fn api_v1_get_subsystem_operator_state(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    if pea_id == DEFAULT_ECLSS_PEA_ID {
        let runtime_state = *context.eclss_runtime.read().await;
        let operator_state = context.eclss_operator_state.read().await.clone();
        let service_state = subsystem_service_state(runtime_state, &operator_state);
        return Ok(axum::Json(json!({
            "pea_id": pea_id,
            "service_tag": ECLSS_SERVICE_TAG,
            "operator_state": operator_state,
            "derived_service_state": service_state,
            "derived_state_code": subsystem_packml_state_code(service_state),
            "runtime": runtime_state
        })));
    }
    if pea_id == DEFAULT_SABATIER_PEA_ID {
        let runtime_state = *context.sabatier_runtime.read().await;
        let operator_state = context.sabatier_operator_state.read().await.clone();
        let service_state = subsystem_service_state(runtime_state, &operator_state);
        return Ok(axum::Json(json!({
            "pea_id": pea_id,
            "service_tag": SABATIER_SERVICE_TAG,
            "operator_state": operator_state,
            "derived_service_state": service_state,
            "derived_state_code": subsystem_packml_state_code(service_state),
            "runtime": runtime_state
        })));
    }
    if pea_id == DEFAULT_POWER_PEA_ID {
        let runtime_state = *context.power_runtime.read().await;
        let operator_state = context.power_operator_state.read().await.clone();
        let service_state = subsystem_service_state(runtime_state, &operator_state);
        return Ok(axum::Json(json!({
            "pea_id": pea_id,
            "service_tag": POWER_SERVICE_TAG,
            "operator_state": operator_state,
            "derived_service_state": service_state,
            "derived_state_code": subsystem_packml_state_code(service_state),
            "runtime": runtime_state
        })));
    }
    if pea_id == DEFAULT_THERMAL_PEA_ID {
        let runtime_state = *context.thermal_runtime.read().await;
        let operator_state = context.thermal_operator_state.read().await.clone();
        let service_state = subsystem_service_state(runtime_state, &operator_state);
        return Ok(axum::Json(json!({
            "pea_id": pea_id,
            "service_tag": THERMAL_SERVICE_TAG,
            "operator_state": operator_state,
            "derived_service_state": service_state,
            "derived_state_code": subsystem_packml_state_code(service_state),
            "runtime": runtime_state
        })));
    }
    if pea_id == DEFAULT_WATER_PEA_ID {
        let runtime_state = *context.water_runtime.read().await;
        let operator_state = context.water_operator_state.read().await.clone();
        let service_state = subsystem_service_state(runtime_state, &operator_state);
        return Ok(axum::Json(json!({
            "pea_id": pea_id,
            "service_tag": WATER_SERVICE_TAG,
            "operator_state": operator_state,
            "derived_service_state": service_state,
            "derived_state_code": subsystem_packml_state_code(service_state),
            "runtime": runtime_state
        })));
    }
    if pea_id == DEFAULT_SAFETY_PEA_ID {
        let runtime_state = *context.safety_runtime.read().await;
        let operator_state = context.safety_operator_state.read().await.clone();
        let service_state = subsystem_service_state(runtime_state, &operator_state);
        return Ok(axum::Json(json!({
            "pea_id": pea_id,
            "service_tag": SAFETY_SERVICE_TAG,
            "operator_state": operator_state,
            "derived_service_state": service_state,
            "derived_state_code": subsystem_packml_state_code(service_state),
            "runtime": runtime_state
        })));
    }
    Err((
        StatusCode::NOT_FOUND,
        format!("Subsystem operator-state not available for PEA {pea_id}"),
    ))
}

async fn api_v1_set_subsystem_operator_state(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<SubsystemOperatorStateUpdateRequest>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    let Some((runtime_handle, operator_handle)) = subsystem_control_handles(&context, &pea_id)
    else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Subsystem operator-state not available for PEA {pea_id}"),
        ));
    };
    let operator_state = {
        let mut state = operator_handle.write().await;
        apply_operator_state_update(&mut state, payload);
        state.clone()
    };
    let runtime_state = {
        let mut runtime = runtime_handle.write().await;
        if !operator_state.command_en
            || matches!(
                operator_state.operation_mode,
                OperationMode::Off | OperationMode::Maint
            )
        {
            runtime.running = false;
            runtime.last_transition_ms = Simulation::now_ms();
        }
        *runtime
    };
    let service_tag = definition_for(&pea_id)
        .map(|definition| definition.service_tag)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")))?;
    let (process_values, timestamp_ms) =
        current_subsystem_process_values(&context, &pea_id).await?;

    let service_state = subsystem_service_state(runtime_state, &operator_state);
    publish_subsystem_uns(
        &context,
        &pea_id,
        service_tag,
        service_state,
        runtime_state,
        timestamp_ms,
        process_values,
    )
    .await;

    journal_operation(
        &context,
        "operator_state_changed",
        pea_id.clone(),
        json!({
            "service_tag": service_tag,
            "operator_state": &operator_state,
            "derived_service_state": service_state,
            "runtime": runtime_state
        }),
    )
    .await;

    Ok(axum::Json(json!({
        "pea_id": pea_id,
        "service_tag": service_tag,
        "operator_state": operator_state,
        "derived_service_state": service_state,
        "derived_state_code": subsystem_packml_state_code(service_state),
        "runtime": runtime_state
    })))
}

fn apply_operator_state_update(
    state: &mut SubsystemOperatorState,
    payload: SubsystemOperatorStateUpdateRequest,
) {
    if let Some(value) = payload.operation_mode {
        state.operation_mode = value;
    }
    if let Some(value) = payload.source_mode {
        state.source_mode = value;
    }
    if let Some(value) = payload.command_en {
        state.command_en = value;
    }
    if let Some(value) = payload.command_en_reason {
        state.command_en_reason = value;
    }
    if let Some(value) = payload.operator_control_enabled {
        state.operator_control_enabled = value;
    }
    if let Some(value) = payload.remote_control_enabled {
        state.remote_control_enabled = value;
    }
    if !state.operator_control_enabled && !state.remote_control_enabled {
        state.source_mode = CommandSourceEnum::SystemAuto;
    } else if !state.operator_control_enabled && state.source_mode == CommandSourceEnum::OperatorUi
    {
        state.source_mode = CommandSourceEnum::RemoteOpcua;
    } else if !state.remote_control_enabled && state.source_mode == CommandSourceEnum::RemoteOpcua {
        state.source_mode = CommandSourceEnum::OperatorUi;
    }
}

fn ensure_subsystem_start_allowed(
    pea_id: &str,
    operator_state: &SubsystemOperatorState,
) -> Result<(), (StatusCode, String)> {
    if !operator_state.command_en {
        let reason = if operator_state.command_en_reason.trim().is_empty() {
            "command enable is false"
        } else {
            operator_state.command_en_reason.as_str()
        };
        return Err((
            StatusCode::CONFLICT,
            format!("PEA {pea_id} start rejected: {reason}"),
        ));
    }
    if matches!(
        operator_state.operation_mode,
        OperationMode::Off | OperationMode::Maint
    ) {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "PEA {pea_id} start rejected in {:?} mode",
                operator_state.operation_mode
            ),
        ));
    }
    Ok(())
}

fn reconcile_runtime_with_operator(
    runtime: &mut PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
    transition_ms: u64,
) {
    if runtime.running
        && (!operator_state.command_en
            || matches!(
                operator_state.operation_mode,
                OperationMode::Off | OperationMode::Maint
            ))
    {
        runtime.running = false;
        runtime.last_transition_ms = transition_ms;
    }
}

fn subsystem_control_handles(
    context: &AppContext,
    pea_id: &str,
) -> Option<(
    Arc<RwLock<PeaRuntimeState>>,
    Arc<RwLock<SubsystemOperatorState>>,
)> {
    match pea_id {
        DEFAULT_ECLSS_PEA_ID => Some((
            context.eclss_runtime.clone(),
            context.eclss_operator_state.clone(),
        )),
        DEFAULT_SABATIER_PEA_ID => Some((
            context.sabatier_runtime.clone(),
            context.sabatier_operator_state.clone(),
        )),
        DEFAULT_POWER_PEA_ID => Some((
            context.power_runtime.clone(),
            context.power_operator_state.clone(),
        )),
        DEFAULT_THERMAL_PEA_ID => Some((
            context.thermal_runtime.clone(),
            context.thermal_operator_state.clone(),
        )),
        DEFAULT_WATER_PEA_ID => Some((
            context.water_runtime.clone(),
            context.water_operator_state.clone(),
        )),
        DEFAULT_SAFETY_PEA_ID => Some((
            context.safety_runtime.clone(),
            context.safety_operator_state.clone(),
        )),
        _ => None,
    }
}

async fn current_subsystem_process_values(
    context: &AppContext,
    pea_id: &str,
) -> Result<(serde_json::Value, u64), (StatusCode, String)> {
    macro_rules! snapshot_value {
        ($snapshot:expr) => {{
            let snapshot = $snapshot;
            (
                serde_json::to_value(&snapshot).unwrap_or_else(|_| json!({})),
                snapshot.timestamp_ms,
            )
        }};
    }
    let result = match pea_id {
        DEFAULT_ECLSS_PEA_ID => snapshot_value!(context.eclss_sim.read().await.snapshot()),
        DEFAULT_SABATIER_PEA_ID => snapshot_value!(context.sabatier_sim.read().await.snapshot()),
        DEFAULT_POWER_PEA_ID => snapshot_value!(context.power_sim.read().await.snapshot()),
        DEFAULT_THERMAL_PEA_ID => snapshot_value!(context.thermal_sim.read().await.snapshot()),
        DEFAULT_WATER_PEA_ID => snapshot_value!(context.water_sim.read().await.snapshot()),
        DEFAULT_SAFETY_PEA_ID => snapshot_value!(context.safety_sim.read().await.snapshot()),
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("Subsystem process values not available for PEA {pea_id}"),
            ));
        }
    };
    Ok(result)
}

async fn api_v1_i3x_list_peas(State(context): State<AppContext>) -> impl IntoResponse {
    let (snapshot, runtime_state) = {
        let sim = context.sim.read().await;
        let runtime_state = *context.airlock_runtime.read().await;
        (sim.snapshot(), runtime_state)
    };
    let eclss_runtime = *context.eclss_runtime.read().await;
    let eclss_operator_state = context.eclss_operator_state.read().await.clone();
    let eclss_snapshot = context.eclss_sim.read().await.snapshot();
    let sabatier_runtime = *context.sabatier_runtime.read().await;
    let sabatier_operator_state = context.sabatier_operator_state.read().await.clone();
    let sabatier_snapshot = context.sabatier_sim.read().await.snapshot();
    let power_runtime = *context.power_runtime.read().await;
    let power_operator_state = context.power_operator_state.read().await.clone();
    let power_snapshot = context.power_sim.read().await.snapshot();
    let thermal_runtime = *context.thermal_runtime.read().await;
    let thermal_operator_state = context.thermal_operator_state.read().await.clone();
    let thermal_snapshot = context.thermal_sim.read().await.snapshot();
    let water_runtime = *context.water_runtime.read().await;
    let water_operator_state = context.water_operator_state.read().await.clone();
    let water_snapshot = context.water_sim.read().await.snapshot();
    let safety_runtime = *context.safety_runtime.read().await;
    let safety_operator_state = context.safety_operator_state.read().await.clone();
    let safety_snapshot = context.safety_sim.read().await.snapshot();
    let item = build_i3x_pea_descriptor(&snapshot, runtime_state, &context.node_id);
    let eclss_item = build_i3x_subsystem_descriptor(
        &context,
        DEFAULT_ECLSS_PEA_ID,
        ECLSS_SERVICE_TAG,
        eclss_runtime,
        &eclss_operator_state,
        &eclss_snapshot,
    );
    let sabatier_item = build_i3x_subsystem_descriptor(
        &context,
        DEFAULT_SABATIER_PEA_ID,
        SABATIER_SERVICE_TAG,
        sabatier_runtime,
        &sabatier_operator_state,
        &sabatier_snapshot,
    );
    let power_item = build_i3x_subsystem_descriptor(
        &context,
        DEFAULT_POWER_PEA_ID,
        POWER_SERVICE_TAG,
        power_runtime,
        &power_operator_state,
        &power_snapshot,
    );
    let thermal_item = build_i3x_subsystem_descriptor(
        &context,
        DEFAULT_THERMAL_PEA_ID,
        THERMAL_SERVICE_TAG,
        thermal_runtime,
        &thermal_operator_state,
        &thermal_snapshot,
    );
    let water_item = build_i3x_subsystem_descriptor(
        &context,
        DEFAULT_WATER_PEA_ID,
        WATER_SERVICE_TAG,
        water_runtime,
        &water_operator_state,
        &water_snapshot,
    );
    let safety_item = build_i3x_subsystem_descriptor(
        &context,
        DEFAULT_SAFETY_PEA_ID,
        SAFETY_SERVICE_TAG,
        safety_runtime,
        &safety_operator_state,
        &safety_snapshot,
    );
    axum::Json(json!({
        "adapter": {
            "name": "underhill-i3x-adapter",
            "version": "0.1.0",
        },
        "items": [item, eclss_item, sabatier_item, power_item, thermal_item, water_item, safety_item],
        "count": 7
    }))
}

async fn api_v1_i3x_get_pea(
    Path(pea_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    if pea_id == DEFAULT_AIRLOCK_PEA_ID {
        let (snapshot, runtime_state) = {
            let sim = context.sim.read().await;
            let runtime_state = *context.airlock_runtime.read().await;
            (sim.snapshot(), runtime_state)
        };
        return Ok(axum::Json(build_i3x_pea_descriptor(
            &snapshot,
            runtime_state,
            &context.node_id,
        )));
    }
    if pea_id == DEFAULT_ECLSS_PEA_ID {
        let runtime_state = *context.eclss_runtime.read().await;
        let operator_state = context.eclss_operator_state.read().await.clone();
        let snapshot = context.eclss_sim.read().await.snapshot();
        return Ok(axum::Json(build_i3x_subsystem_descriptor(
            &context,
            DEFAULT_ECLSS_PEA_ID,
            ECLSS_SERVICE_TAG,
            runtime_state,
            &operator_state,
            &snapshot,
        )));
    }
    if pea_id == DEFAULT_SABATIER_PEA_ID {
        let runtime_state = *context.sabatier_runtime.read().await;
        let operator_state = context.sabatier_operator_state.read().await.clone();
        let snapshot = context.sabatier_sim.read().await.snapshot();
        return Ok(axum::Json(build_i3x_subsystem_descriptor(
            &context,
            DEFAULT_SABATIER_PEA_ID,
            SABATIER_SERVICE_TAG,
            runtime_state,
            &operator_state,
            &snapshot,
        )));
    }
    if pea_id == DEFAULT_POWER_PEA_ID {
        let runtime_state = *context.power_runtime.read().await;
        let operator_state = context.power_operator_state.read().await.clone();
        let snapshot = context.power_sim.read().await.snapshot();
        return Ok(axum::Json(build_i3x_subsystem_descriptor(
            &context,
            DEFAULT_POWER_PEA_ID,
            POWER_SERVICE_TAG,
            runtime_state,
            &operator_state,
            &snapshot,
        )));
    }
    if pea_id == DEFAULT_THERMAL_PEA_ID {
        let runtime_state = *context.thermal_runtime.read().await;
        let operator_state = context.thermal_operator_state.read().await.clone();
        let snapshot = context.thermal_sim.read().await.snapshot();
        return Ok(axum::Json(build_i3x_subsystem_descriptor(
            &context,
            DEFAULT_THERMAL_PEA_ID,
            THERMAL_SERVICE_TAG,
            runtime_state,
            &operator_state,
            &snapshot,
        )));
    }
    if pea_id == DEFAULT_WATER_PEA_ID {
        let runtime_state = *context.water_runtime.read().await;
        let operator_state = context.water_operator_state.read().await.clone();
        let snapshot = context.water_sim.read().await.snapshot();
        return Ok(axum::Json(build_i3x_subsystem_descriptor(
            &context,
            DEFAULT_WATER_PEA_ID,
            WATER_SERVICE_TAG,
            runtime_state,
            &operator_state,
            &snapshot,
        )));
    }
    if pea_id == DEFAULT_SAFETY_PEA_ID {
        let runtime_state = *context.safety_runtime.read().await;
        let operator_state = context.safety_operator_state.read().await.clone();
        let snapshot = context.safety_sim.read().await.snapshot();
        return Ok(axum::Json(build_i3x_subsystem_descriptor(
            &context,
            DEFAULT_SAFETY_PEA_ID,
            SAFETY_SERVICE_TAG,
            runtime_state,
            &operator_state,
            &snapshot,
        )));
    }
    Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")))
}

async fn api_v1_i3x_capability_schema() -> impl IntoResponse {
    axum::Json(json!({
        "adapter": {
            "name": "underhill-i3x-adapter",
            "version": "0.1.0",
        },
        "resource_type": "PEA",
        "state_model": {
            "name": "MTP_S88_MINIMUM",
            "states": [
                "Idle", "Starting", "Execute", "Completing", "Completed",
                "Pausing", "Paused", "Resuming", "Holding", "Held",
                "Unholding", "Stopping", "Stopped", "Aborting", "Aborted", "Resetting"
            ],
            "state_code_map": {
                "Idle": 16,
                "Starting": 8,
                "Execute": 64,
                "Completing": 65536,
                "Completed": 131072,
                "Pausing": 8192,
                "Paused": 32,
                "Resuming": 16384,
                "Holding": 1024,
                "Held": 2048,
                "Unholding": 4096,
                "Stopping": 128,
                "Stopped": 4,
                "Aborting": 256,
                "Aborted": 512,
                "Resetting": 32768
            }
        },
        "commandability": {
            "allowed_commands": [
                "Reset", "Start", "Stop", "Hold", "Unhold",
                "Pause", "Resume", "Abort", "Restart", "Complete"
            ],
            "dispatch_contract": "POST /api/v1/pea/{id}/services/{service_tag}/command"
        }
    }))
}

async fn api_v1_i3x_namespaces() -> impl IntoResponse {
    axum::Json(json!([
        {
            "uri": "https://underhill.murph/ns/pea",
            "displayName": "Underhill PEA Equipment"
        },
        {
            "uri": "https://www.i3x.org/relationships",
            "displayName": "I3X Standard Relationships"
        }
    ]))
}

fn i3x_object_types() -> Vec<serde_json::Value> {
    vec![
        json!({
            "elementId": "BaseEquipment",
            "displayName": "Base Equipment Type",
            "namespaceUri": "https://underhill.murph/ns/pea",
            "schema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "node_id": { "type": "string" }
                }
            }
        }),
        json!({
            "elementId": "PEAType",
            "displayName": "Process Equipment Asset",
            "namespaceUri": "https://underhill.murph/ns/pea",
            "schema": {
                "type": "object",
                "properties": {
                    "pea_id": { "type": "string" },
                    "pea_type": { "type": "string" },
                    "services": { "type": "array" },
                    "opcua_endpoint": { "type": "string" }
                }
            }
        }),
        json!({
            "elementId": "ServiceType",
            "displayName": "PEA Service",
            "namespaceUri": "https://underhill.murph/ns/pea",
            "schema": {
                "type": "object",
                "properties": {
                    "service_tag": { "type": "string" },
                    "state": { "type": "string" }
                }
            }
        }),
    ]
}

async fn api_v1_i3x_objecttypes(Query(query): Query<HashMap<String, String>>) -> impl IntoResponse {
    let namespace_filter = query.get("namespaceUri").map(String::as_str);
    let object_types = i3x_object_types();
    let filtered = if let Some(namespace_uri) = namespace_filter {
        object_types
            .into_iter()
            .filter(|obj| {
                obj.get("namespaceUri")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|ns| ns == namespace_uri)
            })
            .collect::<Vec<_>>()
    } else {
        object_types
    };
    axum::Json(serde_json::Value::Array(filtered))
}

async fn api_v1_i3x_objecttype_by_id(
    Path(element_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    for object_type in i3x_object_types() {
        if object_type
            .get("elementId")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id == element_id)
        {
            return Ok(axum::Json(object_type));
        }
    }
    Err((
        StatusCode::NOT_FOUND,
        format!("Object type not found: {element_id}"),
    ))
}

fn i3x_relationship_types() -> Vec<serde_json::Value> {
    vec![
        json!({
            "elementId": "HasParent",
            "displayName": "Has Parent",
            "namespaceUri": "https://www.i3x.org/relationships",
            "reverseOf": "HasChildren"
        }),
        json!({
            "elementId": "HasChildren",
            "displayName": "Has Children",
            "namespaceUri": "https://www.i3x.org/relationships",
            "reverseOf": "HasParent"
        }),
    ]
}

async fn api_v1_i3x_relationshiptypes() -> impl IntoResponse {
    axum::Json(serde_json::Value::Array(i3x_relationship_types()))
}

async fn api_v1_i3x_relationshiptype_by_id(
    Path(element_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    for rel in i3x_relationship_types() {
        if rel
            .get("elementId")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id == element_id)
        {
            return Ok(axum::Json(rel));
        }
    }
    Err((
        StatusCode::NOT_FOUND,
        format!("Relationship type not found: {element_id}"),
    ))
}

async fn api_v1_i3x_objects(Query(query): Query<HashMap<String, String>>) -> impl IntoResponse {
    let namespace_filter = query.get("namespaceUri").map(String::as_str);
    let objects = i3x_object_instances();
    let filtered = if let Some(namespace_uri) = namespace_filter {
        objects
            .into_iter()
            .filter(|obj| {
                obj.get("namespaceUri")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|ns| ns == namespace_uri)
            })
            .collect::<Vec<_>>()
    } else {
        objects
    };
    axum::Json(serde_json::Value::Array(filtered))
}

async fn api_v1_i3x_object_by_id(
    Path(element_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    for obj in i3x_object_instances() {
        if obj
            .get("elementId")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id == element_id)
        {
            return Ok(axum::Json(obj));
        }
    }
    Err((
        StatusCode::NOT_FOUND,
        format!("Object not found: {element_id}"),
    ))
}

async fn api_v1_i3x_related_objects(
    Path(element_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let relation_filter = query.get("relationshiptype").map(String::as_str);
    let mut related = Vec::new();
    let objects = i3x_object_instances();

    if element_id == "underhill-base" {
        for definition in ALL_PEA_DEFINITIONS {
            let pea_id = definition.pea_id;
            if let Some(instance) = objects.iter().find(|obj| {
                obj.get("elementId")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| id == pea_id)
            }) {
                related.push(with_relationship(
                    instance.clone(),
                    "underhill-base",
                    "HasChildren",
                    "HasParent",
                ));
            }
        }
    } else if let Some(service_tag) = service_tag_for_pea_id(&element_id) {
        if let Some(parent) = objects.iter().find(|obj| {
            obj.get("elementId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| id == "underhill-base")
        }) {
            related.push(with_relationship(
                parent.clone(),
                &element_id,
                "HasParent",
                "HasChildren",
            ));
        }
        let service_id = format!("{element_id}:{service_tag}");
        if let Some(service_obj) = objects.iter().find(|obj| {
            obj.get("elementId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| id == service_id)
        }) {
            related.push(with_relationship(
                service_obj.clone(),
                &element_id,
                "HasChildren",
                "HasParent",
            ));
        }
    } else if let Some((pea_id, _service_tag)) = element_id.split_once(':') {
        if let Some(parent_pea) = objects.iter().find(|obj| {
            obj.get("elementId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| id == pea_id)
        }) {
            related.push(with_relationship(
                parent_pea.clone(),
                &element_id,
                "HasParent",
                "HasChildren",
            ));
        }
    }

    if let Some(filter) = relation_filter {
        related.retain(|obj| {
            obj.get("relationshipType")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|rel| rel == filter)
        });
    }
    axum::Json(serde_json::Value::Array(related))
}

async fn api_v1_i3x_object_value(
    Path(element_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let timestamp = Simulation::now_ms();
    let (value, is_composition) = if element_id == "underhill-base" {
        (
            json!({
                "name": "Underhill Base",
                "node_id": context.node_id,
                "pea_count": ALL_PEA_DEFINITIONS.len()
            }),
            true,
        )
    } else if element_id == DEFAULT_AIRLOCK_PEA_ID {
        let (snapshot, runtime_state) = {
            let sim = context.sim.read().await;
            let runtime_state = *context.airlock_runtime.read().await;
            (sim.snapshot(), runtime_state)
        };
        (build_airlock_pea_descriptor(&snapshot, runtime_state), true)
    } else if element_id == DEFAULT_ECLSS_PEA_ID {
        let runtime_state = *context.eclss_runtime.read().await;
        let operator_state = context.eclss_operator_state.read().await.clone();
        let snapshot = context.eclss_sim.read().await.snapshot();
        (
            build_eclss_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
                DEFAULT_ECLSS_PEA_ID,
                ECLSS_SERVICE_TAG,
            ),
            true,
        )
    } else if element_id == DEFAULT_SABATIER_PEA_ID {
        let runtime_state = *context.sabatier_runtime.read().await;
        let operator_state = context.sabatier_operator_state.read().await.clone();
        let snapshot = context.sabatier_sim.read().await.snapshot();
        (
            build_sabatier_pea_descriptor(
                &context,
                &snapshot,
                runtime_state,
                &operator_state,
                DEFAULT_SABATIER_PEA_ID,
                SABATIER_SERVICE_TAG,
            ),
            true,
        )
    } else if element_id == DEFAULT_POWER_PEA_ID {
        let runtime_state = *context.power_runtime.read().await;
        let operator_state = context.power_operator_state.read().await.clone();
        let snapshot = context.power_sim.read().await.snapshot();
        (
            build_power_pea_descriptor(&context, &snapshot, runtime_state, &operator_state),
            true,
        )
    } else if element_id == DEFAULT_THERMAL_PEA_ID {
        let runtime_state = *context.thermal_runtime.read().await;
        let operator_state = context.thermal_operator_state.read().await.clone();
        let snapshot = context.thermal_sim.read().await.snapshot();
        (
            build_thermal_pea_descriptor(&context, &snapshot, runtime_state, &operator_state),
            true,
        )
    } else if element_id == DEFAULT_WATER_PEA_ID {
        let runtime_state = *context.water_runtime.read().await;
        let operator_state = context.water_operator_state.read().await.clone();
        let snapshot = context.water_sim.read().await.snapshot();
        (
            build_water_pea_descriptor(&context, &snapshot, runtime_state, &operator_state),
            true,
        )
    } else if element_id == DEFAULT_SAFETY_PEA_ID {
        let runtime_state = *context.safety_runtime.read().await;
        let operator_state = context.safety_operator_state.read().await.clone();
        let snapshot = context.safety_sim.read().await.snapshot();
        (
            build_safety_pea_descriptor(&context, &snapshot, runtime_state, &operator_state),
            true,
        )
    } else if let Some((pea_id, service_tag)) = element_id.split_once(':') {
        let state = if pea_id == DEFAULT_AIRLOCK_PEA_ID {
            let snapshot = context.sim.read().await.snapshot();
            format!("{:?}", snapshot.mtp_state_machine.current_state)
        } else if pea_id == DEFAULT_ECLSS_PEA_ID {
            let runtime = *context.eclss_runtime.read().await;
            let operator_state = context.eclss_operator_state.read().await.clone();
            subsystem_service_state(runtime, &operator_state).to_string()
        } else if pea_id == DEFAULT_SABATIER_PEA_ID {
            let runtime = *context.sabatier_runtime.read().await;
            let operator_state = context.sabatier_operator_state.read().await.clone();
            subsystem_service_state(runtime, &operator_state).to_string()
        } else if pea_id == DEFAULT_POWER_PEA_ID {
            let runtime = *context.power_runtime.read().await;
            let operator_state = context.power_operator_state.read().await.clone();
            subsystem_service_state(runtime, &operator_state).to_string()
        } else if pea_id == DEFAULT_THERMAL_PEA_ID {
            let runtime = *context.thermal_runtime.read().await;
            let operator_state = context.thermal_operator_state.read().await.clone();
            subsystem_service_state(runtime, &operator_state).to_string()
        } else if pea_id == DEFAULT_WATER_PEA_ID {
            let runtime = *context.water_runtime.read().await;
            let operator_state = context.water_operator_state.read().await.clone();
            subsystem_service_state(runtime, &operator_state).to_string()
        } else if pea_id == DEFAULT_SAFETY_PEA_ID {
            let runtime = *context.safety_runtime.read().await;
            let operator_state = context.safety_operator_state.read().await.clone();
            subsystem_service_state(runtime, &operator_state).to_string()
        } else {
            return Err((
                StatusCode::NOT_FOUND,
                format!("Object not found: {element_id}"),
            ));
        };
        (
            json!({
                "pea_id": pea_id,
                "service_tag": service_tag,
                "state": state
            }),
            false,
        )
    } else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Object not found: {element_id}"),
        ));
    };

    Ok(axum::Json(json!({
        "elementId": element_id,
        "isComposition": is_composition,
        "value": {
            "value": value,
            "quality": "Good",
            "timestamp": timestamp.to_string()
        }
    })))
}

async fn api_v1_i3x_object_history(
    Path(element_id): Path<String>,
    State(context): State<AppContext>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let current = api_v1_i3x_object_value(Path(element_id.clone()), State(context)).await?;
    let vqt = current.0.get("value").cloned().unwrap_or_else(
        || json!({"value": {}, "quality": "Bad", "timestamp": Simulation::now_ms().to_string()}),
    );
    Ok(axum::Json(json!({
        "elementId": element_id,
        "isComposition": current.0.get("isComposition").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "value": [vqt]
    })))
}

async fn api_v1_i3x_put_object_value(
    Path(element_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        format!("I3X write path not implemented for {element_id}"),
    ))
}

fn i3x_object_instances() -> Vec<serde_json::Value> {
    let mut objects = vec![json!({
        "elementId": "underhill-base",
        "displayName": "Underhill Base",
        "typeId": "BaseEquipment",
        "parentId": serde_json::Value::Null,
        "isComposition": true,
        "namespaceUri": "https://underhill.murph/ns/pea"
    })];
    for definition in ALL_PEA_DEFINITIONS {
        objects.push(json!({
            "elementId": definition.pea_id,
            "displayName": definition.name,
            "typeId": "PEAType",
            "parentId": "underhill-base",
            "isComposition": true,
            "namespaceUri": "https://underhill.murph/ns/pea"
        }));
        objects.push(json!({
            "elementId": format!("{}:{}", definition.pea_id, definition.service_tag),
            "displayName": definition.service_tag,
            "typeId": "ServiceType",
            "parentId": definition.pea_id,
            "isComposition": false,
            "namespaceUri": "https://underhill.murph/ns/pea"
        }));
    }
    objects
}

fn service_tag_for_pea_id(pea_id: &str) -> Option<&'static str> {
    definition_for(pea_id).map(|definition| definition.service_tag)
}

fn with_relationship(
    mut object: serde_json::Value,
    subject: &str,
    relationship_type: &str,
    relationship_type_inverse: &str,
) -> serde_json::Value {
    if let Some(map) = object.as_object_mut() {
        map.insert("subject".to_string(), json!(subject));
        map.insert("relationshipType".to_string(), json!(relationship_type));
        map.insert(
            "relationshipTypeInverse".to_string(),
            json!(relationship_type_inverse),
        );
    }
    object
}

async fn api_v1_pea_service_command(
    Path((pea_id, service_tag)): Path<(String, String)>,
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<PeaServiceCommandRequest>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    if pea_id != DEFAULT_AIRLOCK_PEA_ID {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            format!(
                "Service command channel for {pea_id}/{service_tag} is not implemented yet; lifecycle simulation is available"
            ),
        ));
    }

    let runtime_state = *context.airlock_runtime.read().await;
    if !runtime_state.deployed {
        return Err((
            StatusCode::CONFLICT,
            format!("PEA {pea_id} is not deployed"),
        ));
    }
    if !runtime_state.running {
        return Err((StatusCode::CONFLICT, format!("PEA {pea_id} is not running")));
    }

    let source_str = payload
        .source
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| "pol".to_string());

    let (response, snapshot) = {
        let mut sim = context.sim.write().await;
        let expected_id = sim.snapshot().mtp_runtime.pea_information_label.tag_name;
        let expected_service = sim.snapshot().mtp_runtime.service_information.service_name;

        if pea_id != expected_id {
            return Err((StatusCode::NOT_FOUND, format!("PEA not found: {pea_id}")));
        }
        if !service_tag.eq_ignore_ascii_case(&expected_service) {
            return Err((
                StatusCode::NOT_FOUND,
                format!("Service not found for PEA {pea_id}: {service_tag}"),
            ));
        }

        let source = parse_command_source(&source_str).ok_or((
            StatusCode::BAD_REQUEST,
            "Invalid source. Use POL, operator, or remote".to_string(),
        ))?;

        let req = CommandRequestFields {
            sequence_id: payload.sequence_id,
            command: payload.command,
            param1: payload.param1,
            param2: payload.param2,
            execute: payload.execute,
        };
        let response = sim.write_request(source, req);
        let snapshot = sim.snapshot();
        (response, snapshot)
    };

    let _ = context.snapshots_tx.send(snapshot.clone());
    journal_operation(
        &context,
        "service_command_processed",
        pea_id.clone(),
        json!({
            "service_tag": service_tag,
            "source": source_str,
            "sequence_id": payload.sequence_id,
            "command": payload.command,
            "param1": payload.param1,
            "param2": payload.param2,
            "execute": payload.execute,
            "response": &response
        }),
    )
    .await;
    let runtime_state = *context.airlock_runtime.read().await;
    publish_pea_uns(&context, &snapshot, runtime_state).await;
    Ok(axum::Json(json!({
        "pea_id": pea_id,
        "service_tag": service_tag,
        "response": response,
        "active_command": snapshot.active_command,
        "mtp_state_machine": snapshot.mtp_state_machine
    })))
}

async fn api_set_security_profile(
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<SecurityProfileRequest>,
) -> impl IntoResponse {
    let Some(normalized) = Simulation::normalize_security_profile(&payload.profile) else {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(
                "Unsupported security profile. Use NONE, BASIC256SHA256, or BOTH".to_string(),
            ),
        );
    };
    if let Err(err) = context
        .opcua_control
        .set_security_profile(normalized.clone())
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(format!("Failed to apply security profile: {err}")),
        );
    }

    let _transaction = context.plant_transaction.lock().await;
    let snapshot = {
        let mut sim = context.sim.write().await;
        sim.set_security_profile(&normalized);
        sim.snapshot()
    };
    let _ = context.snapshots_tx.send(snapshot.clone());
    journal_operation(
        &context,
        "security_profile_changed",
        DEFAULT_AIRLOCK_PEA_ID.to_string(),
        json!({ "active_security_mode": snapshot.diagnostics.active_security_mode }),
    )
    .await;
    (
        StatusCode::OK,
        axum::Json(snapshot.diagnostics.active_security_mode),
    )
}

async fn api_set_permissions(
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<PermissionsUpdateRequest>,
) -> impl IntoResponse {
    let _transaction = context.plant_transaction.lock().await;
    let snapshot = {
        let mut sim = context.sim.write().await;
        sim.set_permissions(payload);
        sim.snapshot()
    };
    let _ = context.snapshots_tx.send(snapshot.clone());
    journal_operation(
        &context,
        "permissions_changed",
        DEFAULT_AIRLOCK_PEA_ID.to_string(),
        json!({ "permissions": &snapshot.permissions }),
    )
    .await;
    (StatusCode::OK, axum::Json(snapshot.permissions))
}

async fn api_set_modes(
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<MtpModesUpdateRequest>,
) -> impl IntoResponse {
    let _transaction = context.plant_transaction.lock().await;
    let snapshot = {
        let mut sim = context.sim.write().await;
        sim.set_modes(payload);
        sim.snapshot()
    };
    let _ = context.snapshots_tx.send(snapshot.clone());
    journal_operation(
        &context,
        "operating_mode_changed",
        DEFAULT_AIRLOCK_PEA_ID.to_string(),
        json!({
            "mtp_modes": &snapshot.mtp_modes,
            "mtp_state_machine": &snapshot.mtp_state_machine
        }),
    )
    .await;
    (
        StatusCode::OK,
        axum::Json(json!({
            "mtp_modes": snapshot.mtp_modes,
            "mtp_state_machine": snapshot.mtp_state_machine
        })),
    )
}

async fn api_set_leak_rate(
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<LeakRateUpdateRequest>,
) -> impl IntoResponse {
    let _transaction = context.plant_transaction.lock().await;
    let snapshot = {
        let mut sim = context.sim.write().await;
        sim.set_leak_rate(payload);
        sim.snapshot()
    };
    let _ = context.snapshots_tx.send(snapshot.clone());
    journal_operation(
        &context,
        "fault_injected",
        DEFAULT_AIRLOCK_PEA_ID.to_string(),
        json!({ "alarms": &snapshot.alarms }),
    )
    .await;
    (StatusCode::OK, axum::Json(snapshot.alarms))
}

async fn api_set_valve_fault(
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<ValveFaultUpdateRequest>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let _transaction = context.plant_transaction.lock().await;
    let valve = payload.valve.clone();
    let snapshot = {
        let mut sim = context.sim.write().await;
        sim.set_valve_fault(payload)
            .map_err(|error| (StatusCode::BAD_REQUEST, error))?;
        sim.snapshot()
    };
    let result = json!({
        "valve": valve,
        "equalize": {
            "command_pct": snapshot.equalize_valve_command_pct,
            "true_position_pct": snapshot.equalize_valve_pct,
            "sensed_position_pct": snapshot.equalize_valve_sensed_pct,
            "residual_pct": snapshot.equalize_valve_residual_pct,
            "stiction_active": snapshot.equalize_valve_stiction_active
        },
        "vent": {
            "command_pct": snapshot.vent_valve_command_pct,
            "true_position_pct": snapshot.vent_valve_pct,
            "sensed_position_pct": snapshot.vent_valve_sensed_pct,
            "residual_pct": snapshot.vent_valve_residual_pct,
            "stiction_active": snapshot.vent_valve_stiction_active
        }
    });
    journal_operation(
        &context,
        "valve_fault_changed",
        DEFAULT_AIRLOCK_PEA_ID.to_string(),
        result.clone(),
    )
    .await;
    let _ = context.snapshots_tx.send(snapshot);
    Ok(axum::Json(result))
}

async fn api_write_command(
    Path(source): Path<String>,
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<CommandRequestFields>,
) -> Result<axum::Json<CommandResponseFields>, (StatusCode, String)> {
    let source = parse_command_source(&source)
        .ok_or((StatusCode::BAD_REQUEST, "invalid source path".to_string()))?;
    let _transaction = context.plant_transaction.lock().await;

    let journal_request = payload.clone();
    let (response, snapshot) = {
        let mut sim = context.sim.write().await;
        let response = sim.write_request(source, payload);
        let snapshot = sim.snapshot();
        (response, snapshot)
    };

    let _ = context.snapshots_tx.send(snapshot);
    journal_operation(
        &context,
        "command_processed",
        DEFAULT_AIRLOCK_PEA_ID.to_string(),
        json!({ "source": source, "request": journal_request, "response": &response }),
    )
    .await;
    Ok(axum::Json(response))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(context): State<AppContext>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_client(socket, addr, context))
}

async fn ws_client(socket: axum::extract::ws::WebSocket, addr: SocketAddr, context: AppContext) {
    let session_id = context.next_client_id.fetch_add(1, Ordering::Relaxed);

    {
        let mut sim = context.sim.write().await;
        sim.register_client(
            session_id,
            format!("ws-session-{session_id}"),
            addr.to_string(),
        );
        let _ = context.snapshots_tx.send(sim.snapshot());
    }

    let (mut sender, mut receiver) = socket.split();
    let mut rx = context.systems_snapshots_tx.subscribe();

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(message)) => {
                        if matches!(message, Message::Close(_)) {
                            break;
                        }
                        let mut sim = context.sim.write().await;
                        sim.touch_client(session_id);
                    }
                    Some(Err(_)) | None => break,
                }
            }
            outgoing = rx.recv() => {
                match outgoing {
                    Ok(systems_snapshot) => {
                        let text = match serde_json::to_string(&systems_snapshot) {
                            Ok(value) => value,
                            Err(_) => continue,
                        };
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    {
        let mut sim = context.sim.write().await;
        sim.unregister_client(session_id);
        let _ = context.snapshots_tx.send(sim.snapshot());
    }
}

fn parse_command_source(input: &str) -> Option<CommandSourceEnum> {
    match input.to_ascii_lowercase().as_str() {
        "operator" | "operator_ui" | "pol" => Some(CommandSourceEnum::OperatorUi),
        "remote" | "remote_opcua" => Some(CommandSourceEnum::RemoteOpcua),
        _ => None,
    }
}

fn build_airlock_pea_descriptor(
    snapshot: &Snapshot,
    runtime_state: PeaRuntimeState,
) -> serde_json::Value {
    let service_tag = snapshot
        .mtp_runtime
        .service_information
        .service_name
        .clone();
    let state = format!("{:?}", snapshot.mtp_state_machine.current_state).to_lowercase();

    json!({
        "pea_id": snapshot.mtp_runtime.pea_information_label.tag_name,
        "pea_type": "AIRLOCK",
        "name": snapshot.mtp_runtime.pea_information_label.module_name,
        "node_id": std::env::var("MURPH_NODE_ID").unwrap_or_else(|_| DEFAULT_NODE_ID.to_string()),
        "namespace_uri": "urn:mars-airlock:mtp",
        "root_path": "Objects/MarsBase/AirlockPEA",
        "opcua_endpoint": snapshot.diagnostics.endpoint_url,
        "health_state": snapshot.mtp_runtime.pea_information_label.health_state,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "active_command_running": snapshot.active_command.state == model::CommandStatusEnum::Running,
        "services": [{
            "tag": service_tag,
            "state": state,
            "transition_active": snapshot.mtp_state_machine.transition_active,
            "active_procedure": snapshot.mtp_runtime.service_information.active_procedure,
            "command_en": snapshot.mtp_modes.command_en
        }],
        "updated_at_ms": snapshot.timestamp_ms,
        "last_transition_ms": runtime_state.last_transition_ms
    })
}

fn build_eclss_pea_descriptor(
    context: &AppContext,
    snapshot: &EclssSnapshot,
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
    pea_id: &str,
    service_tag: &str,
) -> serde_json::Value {
    let service_state = subsystem_service_state(runtime_state, operator_state).to_lowercase();
    let health_state = if snapshot.alarm_high_co2 || snapshot.alarm_low_o2 {
        "WARN"
    } else {
        "OK"
    };
    json!({
        "pea_id": pea_id,
        "pea_type": "ECLSS",
        "name": "Underhill ECLSS",
        "node_id": context.node_id.clone(),
        "namespace_uri": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), pea_id),
        "root_path": "Objects/Underhill/ECLSSPEA",
        "opcua_endpoint": context.pea_opcua_endpoints.get(pea_id).cloned().unwrap_or_default(),
        "health_state": health_state,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "active_command_running": false,
        "services": [{
            "tag": service_tag,
            "state": service_state,
            "transition_active": false,
            "active_procedure": if runtime_state.running { "Proc_LifeSupportNominal" } else { "None" },
            "command_en": operator_state.command_en
        }],
        "operator_state": operator_state,
        "process_values": {
            "cabin_pressure_kpa": snapshot.cabin_pressure_kpa,
            "o2_percent": snapshot.o2_percent,
            "co2_ppm": snapshot.co2_ppm,
            "humidity_pct": snapshot.humidity_pct,
            "water_recovery_pct": snapshot.water_recovery_pct,
            "co2_capture_kgph": snapshot.co2_capture_kgph,
            "o2_generation_kgph": snapshot.o2_generation_kgph,
            "power_kw": snapshot.power_kw
        },
        "updated_at_ms": snapshot.timestamp_ms,
        "last_transition_ms": runtime_state.last_transition_ms
    })
}

fn build_sabatier_pea_descriptor(
    context: &AppContext,
    snapshot: &SabatierSnapshot,
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
    pea_id: &str,
    service_tag: &str,
) -> serde_json::Value {
    let service_state = subsystem_service_state(runtime_state, operator_state).to_lowercase();
    let health_state = if snapshot.alarm_reactor_temp {
        "WARN"
    } else {
        "OK"
    };
    json!({
        "pea_id": pea_id,
        "pea_type": "ISRU_SABATIER",
        "name": "Underhill Sabatier",
        "node_id": context.node_id.clone(),
        "namespace_uri": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), pea_id),
        "root_path": "Objects/Underhill/SabatierPEA",
        "opcua_endpoint": context.pea_opcua_endpoints.get(pea_id).cloned().unwrap_or_default(),
        "health_state": health_state,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "active_command_running": false,
        "services": [{
            "tag": service_tag,
            "state": service_state,
            "transition_active": false,
            "active_procedure": if runtime_state.running { "Proc_SabatierNominal" } else { "None" },
            "command_en": operator_state.command_en
        }],
        "operator_state": operator_state,
        "process_values": {
            "reactor_temp_c": snapshot.reactor_temp_c,
            "reactor_pressure_bar": snapshot.reactor_pressure_bar,
            "co2_feed_kgph": snapshot.co2_feed_kgph,
            "h2_feed_kgph": snapshot.h2_feed_kgph,
            "conversion_efficiency_pct": snapshot.conversion_efficiency_pct,
            "methane_production_kgph": snapshot.methane_production_kgph,
            "water_production_kgph": snapshot.water_production_kgph,
            "catalyst_health_pct": snapshot.catalyst_health_pct,
            "power_kw": snapshot.power_kw
        },
        "updated_at_ms": snapshot.timestamp_ms,
        "last_transition_ms": runtime_state.last_transition_ms
    })
}

fn build_power_pea_descriptor(
    context: &AppContext,
    snapshot: &PowerSnapshot,
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
) -> serde_json::Value {
    let service_state = subsystem_service_state(runtime_state, operator_state).to_lowercase();
    let health_state = if snapshot.alarm_bus_undervoltage {
        "FAULT"
    } else if snapshot.alarm_battery_low || snapshot.load_shed_active {
        "WARN"
    } else {
        "OK"
    };
    json!({
        "pea_id": DEFAULT_POWER_PEA_ID,
        "pea_type": "POWER_MICROGRID",
        "name": "Underhill Power Microgrid",
        "node_id": context.node_id.clone(),
        "namespace_uri": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_POWER_PEA_ID),
        "root_path": "Objects/Underhill/PowerPEA",
        "opcua_endpoint": context.pea_opcua_endpoints.get(DEFAULT_POWER_PEA_ID).cloned().unwrap_or_default(),
        "health_state": health_state,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "active_command_running": false,
        "services": [{
            "tag": POWER_SERVICE_TAG,
            "state": service_state,
            "transition_active": false,
            "active_procedure": if runtime_state.running { "Proc_MicrogridNominal" } else { "None" },
            "command_en": operator_state.command_en
        }],
        "operator_state": operator_state,
        "process_values": snapshot,
        "updated_at_ms": snapshot.timestamp_ms,
        "last_transition_ms": runtime_state.last_transition_ms
    })
}

fn build_thermal_pea_descriptor(
    context: &AppContext,
    snapshot: &ThermalSnapshot,
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
) -> serde_json::Value {
    let service_state = subsystem_service_state(runtime_state, operator_state).to_lowercase();
    let health_state = if snapshot.alarm_habitat_hot || snapshot.alarm_habitat_cold {
        "FAULT"
    } else if snapshot.alarm_coolant_hot || !snapshot.cooling_available {
        "WARN"
    } else {
        "OK"
    };
    json!({
        "pea_id": DEFAULT_THERMAL_PEA_ID,
        "pea_type": "THERMAL_CONTROL",
        "name": "Underhill Thermal Control",
        "node_id": context.node_id.clone(),
        "namespace_uri": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_THERMAL_PEA_ID),
        "root_path": "Objects/Underhill/ThermalPEA",
        "opcua_endpoint": context.pea_opcua_endpoints.get(DEFAULT_THERMAL_PEA_ID).cloned().unwrap_or_default(),
        "health_state": health_state,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "active_command_running": false,
        "services": [{
            "tag": THERMAL_SERVICE_TAG,
            "state": service_state,
            "transition_active": false,
            "active_procedure": if runtime_state.running { "Proc_ThermalNominal" } else { "None" },
            "command_en": operator_state.command_en
        }],
        "operator_state": operator_state,
        "process_values": snapshot,
        "updated_at_ms": snapshot.timestamp_ms,
        "last_transition_ms": runtime_state.last_transition_ms
    })
}

fn build_water_pea_descriptor(
    context: &AppContext,
    snapshot: &WaterSnapshot,
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
) -> serde_json::Value {
    let service_state = subsystem_service_state(runtime_state, operator_state).to_lowercase();
    let health_state = if snapshot.unmet_crew_water_kgph > 0.0 || snapshot.alarm_water_quality {
        "FAULT"
    } else if snapshot.alarm_potable_low
        || snapshot.alarm_wastewater_high
        || snapshot.alarm_brine_high
        || !snapshot.treatment_available
    {
        "WARN"
    } else {
        "OK"
    };
    json!({
        "pea_id": DEFAULT_WATER_PEA_ID,
        "pea_type": "WATER_WASTE_RECOVERY",
        "name": "Underhill Water and Waste Recovery",
        "node_id": context.node_id.clone(),
        "namespace_uri": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_WATER_PEA_ID),
        "root_path": "Objects/Underhill/WaterPEA",
        "opcua_endpoint": context.pea_opcua_endpoints.get(DEFAULT_WATER_PEA_ID).cloned().unwrap_or_default(),
        "health_state": health_state,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "active_command_running": false,
        "services": [{
            "tag": WATER_SERVICE_TAG,
            "state": service_state,
            "transition_active": false,
            "active_procedure": if runtime_state.running { "Proc_WaterRecoveryNominal" } else { "None" },
            "command_en": operator_state.command_en
        }],
        "operator_state": operator_state,
        "process_values": snapshot,
        "updated_at_ms": snapshot.timestamp_ms,
        "last_transition_ms": runtime_state.last_transition_ms
    })
}

fn build_safety_pea_descriptor(
    context: &AppContext,
    snapshot: &SafetySnapshot,
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
) -> serde_json::Value {
    let service_state = subsystem_service_state(runtime_state, operator_state).to_lowercase();
    let health_state = if snapshot.alarm_low_pressure
        || snapshot.alarm_rapid_decompression
        || snapshot.alarm_fire
        || snapshot.alarm_toxic_gas
        || snapshot.alarm_structural
    {
        "FAULT"
    } else if snapshot.alarm_radiation || !snapshot.monitoring_available {
        "WARN"
    } else {
        "OK"
    };
    json!({
        "pea_id": DEFAULT_SAFETY_PEA_ID,
        "pea_type": "SAFETY_STRUCTURE",
        "name": "Underhill Safety and Pressure Structure",
        "node_id": context.node_id.clone(),
        "namespace_uri": format!("urn:underhill:{}:pea:{}", context.node_id.as_str(), DEFAULT_SAFETY_PEA_ID),
        "root_path": "Objects/Underhill/SafetyPEA",
        "opcua_endpoint": context.pea_opcua_endpoints.get(DEFAULT_SAFETY_PEA_ID).cloned().unwrap_or_default(),
        "health_state": health_state,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "active_command_running": false,
        "services": [{
            "tag": SAFETY_SERVICE_TAG,
            "state": service_state,
            "transition_active": false,
            "active_procedure": if runtime_state.running { "Proc_SafetyMonitoringNominal" } else { "None" },
            "command_en": operator_state.command_en
        }],
        "operator_state": operator_state,
        "process_values": snapshot,
        "updated_at_ms": snapshot.timestamp_ms,
        "last_transition_ms": runtime_state.last_transition_ms
    })
}

fn build_i3x_pea_descriptor(
    snapshot: &Snapshot,
    runtime_state: PeaRuntimeState,
    node_id: &str,
) -> serde_json::Value {
    let service_tag = snapshot
        .mtp_runtime
        .service_information
        .service_name
        .clone();
    let service_state = format!("{:?}", snapshot.mtp_state_machine.current_state);
    let pea_id = snapshot.mtp_runtime.pea_information_label.tag_name.clone();

    json!({
        "resource_id": format!("pea:{pea_id}"),
        "resource_type": "PEA",
        "pea_id": pea_id,
        "pea_type": "AIRLOCK",
        "name": snapshot.mtp_runtime.pea_information_label.module_name,
        "node_id": node_id,
        "namespace_uri": "urn:mars-airlock:mtp",
        "uns_namespace": format!("murph/habitat/nodes/{node_id}/pea/{}", snapshot.mtp_runtime.pea_information_label.tag_name),
        "opcua_endpoint": snapshot.diagnostics.endpoint_url,
        "health_state": snapshot.mtp_runtime.pea_information_label.health_state,
        "runtime": {
            "deployed": runtime_state.deployed,
            "running": runtime_state.running,
            "last_transition_ms": runtime_state.last_transition_ms,
        },
        "services": [{
            "tag": service_tag,
            "state": service_state,
            "state_code": packml_state_code(snapshot.mtp_state_machine.current_state),
            "active_procedure": snapshot.mtp_runtime.service_information.active_procedure,
            "transition_active": snapshot.mtp_state_machine.transition_active,
            "commandability": {
                "command_en": snapshot.mtp_modes.command_en,
                "dispatch_contract": "POST /api/v1/pea/{id}/services/{service_tag}/command",
                "allowed_commands": [
                    "Reset", "Start", "Stop", "Hold", "Unhold",
                    "Pause", "Resume", "Abort", "Restart", "Complete"
                ],
            }
        }],
        "updated_at_ms": snapshot.timestamp_ms,
    })
}

fn build_i3x_subsystem_descriptor<T: Serialize>(
    context: &AppContext,
    pea_id: &str,
    service_tag: &str,
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
    snapshot: &T,
) -> serde_json::Value {
    let process_values = serde_json::to_value(snapshot).unwrap_or_else(|_| json!({}));
    let service_state = subsystem_service_state(runtime_state, operator_state);
    let state_code = subsystem_packml_state_code(service_state);
    let health_state = health_from_process_values(&process_values);
    let timestamp_ms = process_values
        .get("timestamp_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(Simulation::now_ms);
    let definition = definition_for(pea_id);
    let pea_type = definition
        .map(|definition| definition.pea_type)
        .unwrap_or("PROCESS_EQUIPMENT_ASSEMBLY");
    let name = definition
        .map(|definition| definition.name)
        .unwrap_or("Underhill PEA");
    let uns_namespace = format!(
        "murph/habitat/nodes/{}/pea/{pea_id}",
        context.node_id.as_str()
    );

    json!({
        "resource_id": format!("pea:{pea_id}"),
        "resource_type": "PEA",
        "pea_id": pea_id,
        "pea_type": pea_type,
        "name": name,
        "node_id": context.node_id.clone(),
        "namespace_uri": format!("urn:underhill:{}:pea:{pea_id}", context.node_id.as_str()),
        "uns_namespace": uns_namespace,
        "opcua_endpoint": context.pea_opcua_endpoints.get(pea_id).cloned().unwrap_or_default(),
        "health_state": health_state,
        "runtime": {
            "deployed": runtime_state.deployed,
            "running": runtime_state.running,
            "last_transition_ms": runtime_state.last_transition_ms,
        },
        "services": [{
            "tag": service_tag,
            "state": service_state,
            "state_code": state_code,
            "active_procedure": if runtime_state.running { "Proc_Nominal" } else { "None" },
            "transition_active": false,
            "commandability": {
                "command_en": operator_state.command_en,
                "dispatch_contract": "POST /api/v1/pea/{id}/services/{service_tag}/command",
                "allowed_commands": [],
            }
        }],
        "operator_state": operator_state,
        "process_values": process_values,
        "updated_at_ms": timestamp_ms,
    })
}

fn subsystem_service_state(
    runtime_state: PeaRuntimeState,
    operator_state: &SubsystemOperatorState,
) -> &'static str {
    if !runtime_state.deployed {
        "Stopped"
    } else if matches!(
        operator_state.operation_mode,
        OperationMode::Off | OperationMode::Maint
    ) {
        "Stopped"
    } else if !operator_state.command_en {
        "Held"
    } else if runtime_state.running {
        "Execute"
    } else {
        "Idle"
    }
}

fn subsystem_packml_state_code(state: &str) -> u32 {
    match state {
        "Execute" => 64,
        "Stopped" => 4,
        "Held" => 2048,
        _ => 16,
    }
}

fn health_from_process_values(process_values: &serde_json::Value) -> &'static str {
    let Some(obj) = process_values.as_object() else {
        return "OK";
    };
    for key in [
        "alarm_low_pressure",
        "alarm_rapid_decompression",
        "alarm_fire",
        "alarm_toxic_gas",
        "alarm_structural",
        "alarm_bus_undervoltage",
    ] {
        if obj.get(key).and_then(serde_json::Value::as_bool) == Some(true) {
            return "FAULT";
        }
    }
    for (key, value) in obj {
        if key.starts_with("alarm_") && value.as_bool().unwrap_or(false) {
            return "WARN";
        }
    }
    if obj
        .get("monitoring_available")
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        return "WARN";
    }
    "OK"
}

fn safety_alarm_active(snapshot: &SafetySnapshot) -> bool {
    snapshot.alarm_low_pressure
        || snapshot.alarm_rapid_decompression
        || snapshot.alarm_fire
        || snapshot.alarm_toxic_gas
        || snapshot.alarm_radiation
        || snapshot.alarm_structural
}

fn resolve_opcua_advertised_host() -> String {
    let bind_host = std::env::var("AIRLOCK_OPCUA_BIND_HOST")
        .or_else(|_| std::env::var("AIRLOCK_OPCUA_HOST"))
        .unwrap_or_else(|_| "0.0.0.0".to_string());
    std::env::var("AIRLOCK_OPCUA_HOST").unwrap_or_else(|_| {
        if bind_host == "0.0.0.0" {
            "127.0.0.1".to_string()
        } else {
            bind_host
        }
    })
}

fn build_opcua_endpoint_url(host: &str, port: u16, path: &str) -> String {
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    format!("opc.tcp://{host}:{port}{normalized_path}")
}

fn allocate_opcua_port_for_pea(pea_id: &str, forced_port: Option<u16>) -> anyhow::Result<u16> {
    let (range_min, range_max) = parse_opcua_port_range()?;
    let store_path = opcua_port_store_path();
    let mut store = load_opcua_port_store(&store_path)?;
    validate_store_collisions(&store.allocations)?;

    let assigned_port = if let Some(port) = forced_port {
        if let Some(conflict) = store
            .allocations
            .iter()
            .find(|(existing_pea, existing_port)| {
                existing_pea.as_str() != pea_id && **existing_port == port
            })
        {
            return Err(anyhow::anyhow!(
                "OPC UA port collision: {pea_id} cannot use {}, already allocated to {}",
                port,
                conflict.0
            ));
        }
        store.allocations.insert(pea_id.to_string(), port);
        port
    } else if let Some(existing) = store.allocations.get(pea_id).copied() {
        if existing < range_min || existing > range_max {
            return Err(anyhow::anyhow!(
                "Stored OPC UA port {existing} for {pea_id} is outside configured range {range_min}-{range_max}"
            ));
        }
        existing
    } else {
        let used_ports: HashSet<u16> = store.allocations.values().copied().collect();
        let Some(next_port) = (range_min..=range_max).find(|port| !used_ports.contains(port))
        else {
            return Err(anyhow::anyhow!(
                "No available OPC UA ports in configured range {range_min}-{range_max}"
            ));
        };
        store.allocations.insert(pea_id.to_string(), next_port);
        next_port
    };

    write_opcua_port_store(&store_path, &store)?;
    Ok(assigned_port)
}

fn parse_opcua_port_range() -> anyhow::Result<(u16, u16)> {
    let configured = std::env::var("UNDERHILL_OPCUA_PORT_RANGE").unwrap_or_else(|_| {
        format!("{DEFAULT_OPCUA_PORT_RANGE_MIN}-{DEFAULT_OPCUA_PORT_RANGE_MAX}")
    });
    let mut parts = configured.split('-');
    let start_str = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid UNDERHILL_OPCUA_PORT_RANGE {configured}"))?;
    let end_str = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid UNDERHILL_OPCUA_PORT_RANGE {configured}"))?;
    if parts.next().is_some() {
        return Err(anyhow::anyhow!(
            "Invalid UNDERHILL_OPCUA_PORT_RANGE {configured}; expected start-end"
        ));
    }
    let start = start_str
        .trim()
        .parse::<u16>()
        .map_err(|_| anyhow::anyhow!("Invalid port range start {start_str}"))?;
    let end = end_str
        .trim()
        .parse::<u16>()
        .map_err(|_| anyhow::anyhow!("Invalid port range end {end_str}"))?;
    if start > end {
        return Err(anyhow::anyhow!(
            "Invalid UNDERHILL_OPCUA_PORT_RANGE {configured}; start must be <= end"
        ));
    }
    Ok((start, end))
}

fn opcua_port_store_path() -> PathBuf {
    if let Ok(path) = std::env::var("UNDERHILL_OPCUA_PORT_ALLOCATIONS_FILE") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("opcua_port_allocations.json")
}

fn load_opcua_port_store(path: &FsPath) -> anyhow::Result<OpcuaPortAllocationStore> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let store =
                serde_json::from_str::<OpcuaPortAllocationStore>(&contents).map_err(|err| {
                    anyhow::anyhow!(
                        "Failed to parse OPC UA port allocation file {}: {err}",
                        path.display()
                    )
                })?;
            Ok(store)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(OpcuaPortAllocationStore::default()),
        Err(err) => Err(anyhow::anyhow!(
            "Failed to read OPC UA port allocation file {}: {err}",
            path.display()
        )),
    }
}

fn validate_store_collisions(allocations: &BTreeMap<String, u16>) -> anyhow::Result<()> {
    let mut reverse: HashMap<u16, String> = HashMap::new();
    for (pea_id, port) in allocations {
        if let Some(existing_pea) = reverse.insert(*port, pea_id.clone()) {
            return Err(anyhow::anyhow!(
                "OPC UA port collision in allocation file: port {} assigned to {} and {}",
                port,
                existing_pea,
                pea_id
            ));
        }
    }
    Ok(())
}

fn write_opcua_port_store(path: &FsPath, store: &OpcuaPortAllocationStore) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            anyhow::anyhow!(
                "Failed to create OPC UA allocation directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let payload = serde_json::to_string_pretty(store)
        .map_err(|err| anyhow::anyhow!("Failed to serialize OPC UA allocation store: {err}"))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, payload)
        .map_err(|err| anyhow::anyhow!("Failed to write {}: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path)
        .map_err(|err| anyhow::anyhow!("Failed to move {} into place: {err}", path.display()))?;
    Ok(())
}

async fn open_zenoh_session() -> anyhow::Result<Session> {
    let mut config = zenoh::Config::default();
    if let Ok(endpoint) = std::env::var("ZENOH_ROUTER") {
        config
            .insert_json5("connect/endpoints", &format!(r#"["{}"]"#, endpoint))
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    zenoh::open(config).await.map_err(|e| anyhow::anyhow!(e))
}

async fn publish_pea_uns(
    context: &AppContext,
    snapshot: &Snapshot,
    runtime_state: PeaRuntimeState,
) {
    let pea_id = snapshot.mtp_runtime.pea_information_label.tag_name.clone();
    let service_tag = snapshot
        .mtp_runtime
        .service_information
        .service_name
        .clone();
    let node_id = context.node_id.clone();

    let announce_topic = format!("murph/habitat/nodes/{node_id}/pea/{pea_id}/announce");
    let status_topic = format!("murph/habitat/nodes/{node_id}/pea/{pea_id}/status");
    let service_state_topic =
        format!("murph/habitat/nodes/{node_id}/pea/{pea_id}/services/{service_tag}/state");

    let service_state = format!("{:?}", snapshot.mtp_state_machine.current_state);
    let service_state_code = packml_state_code(snapshot.mtp_state_machine.current_state);
    let announce_payload = json!({
        "pea_id": pea_id,
        "name": snapshot.mtp_runtime.pea_information_label.module_name,
        "version": snapshot.mtp_runtime.pea_information_label.software_revision,
        "services": [{
            "tag": service_tag,
            "name": snapshot.mtp_runtime.service_information.service_name,
        }],
        "opcua_endpoint": snapshot.diagnostics.endpoint_url,
        "timestamp_ms": snapshot.timestamp_ms,
    });
    let status_payload = json!({
        "pea_id": snapshot.mtp_runtime.pea_information_label.tag_name,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "services": [{
            "tag": snapshot.mtp_runtime.service_information.service_name,
            "state": service_state,
            "state_code": service_state_code,
            "current_procedure_id": serde_json::Value::Null,
        }],
        "opcua_endpoint": snapshot.diagnostics.endpoint_url,
        "last_updated": snapshot.timestamp_ms,
    });
    let service_payload = json!({
        "state": format!("{:?}", snapshot.mtp_state_machine.current_state),
        "state_code": service_state_code,
        "current_procedure_id": serde_json::Value::Null,
        "timestamp_ms": snapshot.timestamp_ms,
    });

    if let Some(session) = context.zenoh_session.as_ref() {
        if let Err(err) = session
            .put(&announce_topic, announce_payload.to_string())
            .await
        {
            error!("Failed to publish announce topic {announce_topic}: {err}");
        }
        if let Err(err) = session.put(&status_topic, status_payload.to_string()).await {
            error!("Failed to publish status topic {status_topic}: {err}");
        }
        if let Err(err) = session
            .put(&service_state_topic, service_payload.to_string())
            .await
        {
            error!("Failed to publish service state topic {service_state_topic}: {err}");
        }
    }

    if let Some(mqtt) = context.mqtt_uns.as_ref() {
        if let Err(err) = mqtt.publish_json(&announce_topic, &announce_payload).await {
            error!("Failed to MQTT-publish {announce_topic}: {err}");
        }
        if let Err(err) = mqtt.publish_json(&status_topic, &status_payload).await {
            error!("Failed to MQTT-publish {status_topic}: {err}");
        }
        if let Err(err) = mqtt
            .publish_json(&service_state_topic, &service_payload)
            .await
        {
            error!("Failed to MQTT-publish {service_state_topic}: {err}");
        }
    }
}

async fn publish_subsystem_uns(
    context: &AppContext,
    pea_id: &str,
    service_tag: &str,
    service_state: &str,
    runtime_state: PeaRuntimeState,
    timestamp_ms: u64,
    process_values: serde_json::Value,
) {
    let node_id = context.node_id.clone();
    let announce_topic = format!("murph/habitat/nodes/{node_id}/pea/{pea_id}/announce");
    let status_topic = format!("murph/habitat/nodes/{node_id}/pea/{pea_id}/status");
    let service_state_topic =
        format!("murph/habitat/nodes/{node_id}/pea/{pea_id}/services/{service_tag}/state");

    let state_code = subsystem_packml_state_code(service_state);
    let announce_payload = json!({
        "pea_id": pea_id,
        "name": definition_for(pea_id).map(|definition| definition.name).unwrap_or("Underhill PEA"),
        "version": "0.1.0",
        "services": [{
            "tag": service_tag,
            "name": service_tag,
        }],
        "opcua_endpoint": context.pea_opcua_endpoints.get(pea_id).cloned().unwrap_or_default(),
        "timestamp_ms": timestamp_ms,
    });
    let status_payload = json!({
        "pea_id": pea_id,
        "deployed": runtime_state.deployed,
        "running": runtime_state.running,
        "services": [{
            "tag": service_tag,
            "state": service_state,
            "state_code": state_code,
            "current_procedure_id": serde_json::Value::Null,
        }],
        "opcua_endpoint": context.pea_opcua_endpoints.get(pea_id).cloned().unwrap_or_default(),
        "last_updated": timestamp_ms,
    });
    let service_payload = json!({
        "state": service_state,
        "state_code": state_code,
        "current_procedure_id": serde_json::Value::Null,
        "timestamp_ms": timestamp_ms,
    });

    if let Some(session) = context.zenoh_session.as_ref() {
        if let Err(err) = session
            .put(&announce_topic, announce_payload.to_string())
            .await
        {
            error!("Failed to publish announce topic {announce_topic}: {err}");
        }
        if let Err(err) = session.put(&status_topic, status_payload.to_string()).await {
            error!("Failed to publish status topic {status_topic}: {err}");
        }
        if let Err(err) = session
            .put(&service_state_topic, service_payload.to_string())
            .await
        {
            error!("Failed to publish service state topic {service_state_topic}: {err}");
        }
    }

    if let Some(mqtt) = context.mqtt_uns.as_ref() {
        if let Err(err) = mqtt.publish_json(&announce_topic, &announce_payload).await {
            error!("Failed to MQTT-publish {announce_topic}: {err}");
        }
        if let Err(err) = mqtt.publish_json(&status_topic, &status_payload).await {
            error!("Failed to MQTT-publish {status_topic}: {err}");
        }
        if let Err(err) = mqtt
            .publish_json(&service_state_topic, &service_payload)
            .await
        {
            error!("Failed to MQTT-publish {service_state_topic}: {err}");
        }
    }

    if let Some(values) = process_values.as_object() {
        for (key, value) in values {
            if key == "timestamp_ms" || key.starts_with("alarm_") {
                continue;
            }
            let data_topic = format!("murph/habitat/nodes/{node_id}/pea/{pea_id}/data/{key}");
            if let Some(session) = context.zenoh_session.as_ref() {
                if let Err(err) = session.put(&data_topic, value.to_string()).await {
                    error!("Failed to publish data topic {data_topic}: {err}");
                }
            }
            if let Some(mqtt) = context.mqtt_uns.as_ref() {
                let payload = json!({ "value": value, "timestamp_ms": timestamp_ms });
                if let Err(err) = mqtt.publish_json(&data_topic, &payload).await {
                    error!("Failed to MQTT-publish {data_topic}: {err}");
                }
            }
        }
    }
}

fn packml_state_code(state: model::ServiceState) -> u32 {
    match state {
        model::ServiceState::Idle => 16,
        model::ServiceState::Starting => 8,
        model::ServiceState::Execute => 64,
        model::ServiceState::Completing => 65536,
        model::ServiceState::Completed => 131072,
        model::ServiceState::Pausing => 8192,
        model::ServiceState::Paused => 32,
        model::ServiceState::Resuming => 16384,
        model::ServiceState::Holding => 1024,
        model::ServiceState::Held => 2048,
        model::ServiceState::Unholding => 4096,
        model::ServiceState::Stopping => 128,
        model::ServiceState::Stopped => 4,
        model::ServiceState::Aborting => 256,
        model::ServiceState::Aborted => 512,
        model::ServiceState::Resetting => 32768,
    }
}

// ============================================================================
// MTP Compliance (Phase 2): Parameter Type System & Procedure Calling Convention
// ============================================================================

/// Get MTP manifest (CAEX XML) for a PEA
async fn api_v2_get_manifest(
    Path(pea_id): Path<String>,
) -> Result<(axum::http::StatusCode, String), (StatusCode, String)> {
    let manifest_name = match pea_id.as_str() {
        DEFAULT_AIRLOCK_PEA_ID => "AIRLOCK-MTP-Manifest.aml",
        DEFAULT_ECLSS_PEA_ID => "ECLSS-MTP-Manifest.aml",
        DEFAULT_SABATIER_PEA_ID => "SABATIER-MTP-Manifest.aml",
        _ => return Err((StatusCode::NOT_FOUND, format!("Unknown PEA: {pea_id}"))),
    };

    // Attempt to load from Focus directory
    let manifest_path = PathBuf::from("/home/earthling/Documents/Focus").join(manifest_name);

    match fs::read_to_string(&manifest_path) {
        Ok(content) => Ok((StatusCode::OK, content)),
        Err(_) => {
            // Return stub manifest for now
            Ok((
                StatusCode::OK,
                format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<!-- MTP Manifest for {pea_id} (stub - see /home/earthling/Documents/Focus/{manifest_name}) -->
<CAEXFile xmlns="http://www.plcopen.org/xml/tc6_0201" SchemaVersion="2.15">
  <Description>MTP Manifest for {pea_id}</Description>
</CAEXFile>"#
                ),
            ))
        }
    }
}

/// List services for a PEA
async fn api_v2_get_service(
    Path((pea_id, service_name)): Path<(String, String)>,
    State(_context): State<AppContext>,
) -> Result<axum::Json<ServiceDefinition>, (StatusCode, String)> {
    // For now, return stub definitions. In full implementation, these would be loaded from manifest
    match (pea_id.as_str(), service_name.as_str()) {
        (DEFAULT_AIRLOCK_PEA_ID, "Depressurization") => Ok(axum::Json(ServiceDefinition {
            id: 1,
            name: "Depressurization".to_string(),
            parameters: vec![
                ServiceParameter {
                    name: "target_pressure_pa".to_string(),
                    category: ParameterCategory::ProcedureParameter,
                    data_type: "xs:double".to_string(),
                    unit: Some("Pa".to_string()),
                    min: Some(0.0),
                    max: Some(150000.0),
                    default_value: None,
                    description: Some("Target chamber pressure".to_string()),
                    current_value: Some(json!(3500.0)),
                },
                ServiceParameter {
                    name: "timeout_sec".to_string(),
                    category: ParameterCategory::ProcedureParameter,
                    data_type: "xs:unsignedLong".to_string(),
                    unit: Some("sec".to_string()),
                    min: Some(10.0),
                    max: Some(3600.0),
                    default_value: Some(json!(300)),
                    description: Some("Maximum time for operation".to_string()),
                    current_value: Some(json!(300)),
                },
            ],
            procedures: vec![ServiceProcedure {
                id: 1,
                name: "DepressurizeForEVA".to_string(),
                description: Some("Depressurize to vacuum for EVA".to_string()),
                input_parameters: vec!["target_pressure_pa".to_string(), "timeout_sec".to_string()],
                output_parameters: vec!["final_pressure_pa".to_string()],
            }],
        })),
        _ => Err((
            StatusCode::NOT_FOUND,
            format!("Service {service_name} not found in {pea_id}"),
        )),
    }
}

/// List parameters for a service
async fn api_v2_list_service_parameters(
    Path((pea_id, service_name)): Path<(String, String)>,
    State(context): State<AppContext>,
) -> Result<axum::Json<Vec<ServiceParameter>>, (StatusCode, String)> {
    let service =
        api_v2_get_service(Path((pea_id.clone(), service_name.clone())), State(context)).await?;

    Ok(axum::Json(service.0.parameters))
}

/// Get a specific parameter value
async fn api_v2_get_parameter(
    Path((pea_id, service_name, param_name)): Path<(String, String, String)>,
    State(context): State<AppContext>,
) -> Result<axum::Json<ServiceParameter>, (StatusCode, String)> {
    let service = api_v2_get_service(Path((pea_id, service_name)), State(context)).await?;

    let param = service
        .0
        .parameters
        .iter()
        .find(|p| p.name == param_name)
        .cloned()
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Parameter {param_name} not found"),
        ))?;

    Ok(axum::Json(param))
}

/// Set a parameter value
async fn api_v2_set_parameter(
    Path((pea_id, service_name, param_name)): Path<(String, String, String)>,
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> Result<axum::Json<ServiceParameter>, (StatusCode, String)> {
    let mut service = api_v2_get_service(Path((pea_id, service_name)), State(context))
        .await?
        .0;

    // Update parameter value
    let param = service
        .parameters
        .iter_mut()
        .find(|p| p.name == param_name)
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Parameter {param_name} not found"),
        ))?;

    // Validate value against parameter type/constraints (simplified)
    param.current_value = Some(payload);

    Ok(axum::Json(param.clone()))
}

/// List procedures for a service
async fn api_v2_list_procedures(
    Path((pea_id, service_name)): Path<(String, String)>,
    State(context): State<AppContext>,
) -> Result<axum::Json<Vec<ServiceProcedure>>, (StatusCode, String)> {
    let service = api_v2_get_service(Path((pea_id, service_name)), State(context)).await?;

    Ok(axum::Json(service.0.procedures))
}

/// Get a specific procedure definition
async fn api_v2_get_procedure(
    Path((pea_id, service_name, proc_name)): Path<(String, String, String)>,
    State(context): State<AppContext>,
) -> Result<axum::Json<ServiceProcedure>, (StatusCode, String)> {
    let service = api_v2_get_service(Path((pea_id, service_name)), State(context)).await?;

    let proc = service
        .0
        .procedures
        .iter()
        .find(|p| p.name == proc_name)
        .cloned()
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Procedure {proc_name} not found"),
        ))?;

    Ok(axum::Json(proc))
}

/// Request execution of a procedure (MTP Calling Convention: Request ID != Current ID)
async fn api_v2_request_procedure(
    Path((pea_id, service_name, proc_name)): Path<(String, String, String)>,
    State(context): State<AppContext>,
    axum::Json(payload): axum::Json<ProcedureRequestInput>,
) -> Result<axum::Json<ProcedureStatusResponse>, (StatusCode, String)> {
    // Validate procedure exists
    let _proc = api_v2_get_procedure(
        Path((pea_id.clone(), service_name.clone(), proc_name.clone())),
        State(context.clone()),
    )
    .await?;

    let request_id = payload.request_id;
    let _transaction = context.plant_transaction.lock().await;

    let request = ProcedureRequest {
        request_id,
        procedure_id: 0, // placeholder
        procedure_name: proc_name.clone(),
        service_name: service_name.clone(),
        pea_id: pea_id.clone(),
        parameters: payload.parameters.unwrap_or_default(),
        requested_at_ms: Simulation::now_ms(),
    };

    // Store and journal the request within the same plant transaction.
    {
        let mut sim = context.sim.write().await;
        sim.request_procedure(request.clone());
    }
    journal_operation(
        &context,
        "procedure_requested",
        pea_id,
        serde_json::to_value(request).unwrap_or_else(|_| json!({})),
    )
    .await;

    Ok(axum::Json(ProcedureStatusResponse {
        request_id,
        current_request_id: 0, // Will match request_id when done
        state: ProcedureState::Running,
        progress_pct: 0.0,
        result: HashMap::new(),
        error_message: None,
    }))
}

/// Get status of a procedure execution (MTP Polling: Current ID == Request ID indicates completion)
async fn api_v2_get_procedure_status(
    Path((pea_id, service_name, proc_name, request_id_str)): Path<(String, String, String, String)>,
    State(context): State<AppContext>,
) -> Result<axum::Json<ProcedureStatusResponse>, (StatusCode, String)> {
    let request_id: u32 = request_id_str.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid request_id format".to_string(),
        )
    })?;

    // Validate procedure exists (clone values because they will be reused)
    let _proc = api_v2_get_procedure(
        Path((pea_id.clone(), service_name.clone(), proc_name.clone())),
        State(context.clone()),
    )
    .await?;

    // fetch runtime from simulation
    {
        let sim = context.sim.read().await;
        let key = format!(
            "{}/{}/{}",
            pea_id.clone(),
            service_name.clone(),
            proc_name.clone()
        );
        if let Some(runtime) = sim.procedure_runtimes.get(&key) {
            return Ok(axum::Json(ProcedureStatusResponse {
                request_id,
                current_request_id: runtime.current_request_id,
                state: runtime.state.clone(),
                progress_pct: runtime.progress_pct,
                result: runtime.result.clone(),
                error_message: runtime.error_message.clone(),
            }));
        }
    }
    // if not found, return error
    Err((
        StatusCode::NOT_FOUND,
        format!("Procedure runtime not found for request {}", request_id),
    ))
}
