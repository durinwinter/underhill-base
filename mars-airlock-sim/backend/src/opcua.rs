use std::sync::{Arc, Mutex};
use std::time::Duration;

use opcua::{
    crypto::SecurityPolicy,
    server::{
        ANONYMOUS_USER_TOKEN_ID, ServerBuilder, ServerEndpoint, ServerHandle, SubscriptionCache,
        address_space::{AccessLevel, AddressSpace, Variable},
        diagnostics::NamespaceMetadata,
        node_manager::memory::{SimpleNodeManager, SimpleNodeManagerImpl, simple_node_manager},
    },
    types::{DataValue, MessageSecurityMode, NodeId, StatusCode, VariableId, Variant},
};
use tokio::{
    net::TcpListener,
    sync::{RwLock, broadcast, mpsc, oneshot},
    task::JoinHandle,
};
use tracing::{error, info, warn};

use crate::model::{
    CommandEnum, CommandRequestFields, CommandSourceEnum, CommandStatusEnum, ProcedureRuntime,
    Snapshot,
};
use crate::sim::Simulation;

const NAMESPACE_URI: &str = "urn:mars-airlock:mtp";

#[derive(Clone)]
pub struct OpcuaControl {
    tx: mpsc::Sender<OpcuaControlCommand>,
}

enum OpcuaControlCommand {
    SetSecurityProfile {
        profile: String,
        response: oneshot::Sender<anyhow::Result<()>>,
    },
}

#[derive(Clone, Debug)]
pub struct OpcuaRuntimeConfig {
    bind_host: String,
    host: String,
    port: u16,
    endpoint_path: String,
}

impl OpcuaRuntimeConfig {
    fn from_env() -> Self {
        let bind_host = std::env::var("AIRLOCK_OPCUA_BIND_HOST")
            .or_else(|_| std::env::var("AIRLOCK_OPCUA_HOST"))
            .unwrap_or_else(|_| "0.0.0.0".to_string());
        let host = std::env::var("AIRLOCK_OPCUA_HOST").unwrap_or_else(|_| {
            if bind_host == "0.0.0.0" {
                "127.0.0.1".to_string()
            } else {
                bind_host.clone()
            }
        });
        Self {
            bind_host,
            host,
            port: std::env::var("AIRLOCK_OPCUA_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(4841),
            endpoint_path: std::env::var("AIRLOCK_OPCUA_ENDPOINT_PATH")
                .unwrap_or_else(|_| "/underhill/airlock".to_string()),
        }
    }

    pub fn from_env_with_port(port: u16) -> Self {
        let mut config = Self::from_env();
        config.port = port;
        config
    }

    pub fn endpoint_url(&self) -> String {
        format!(
            "opc.tcp://{}:{}{}",
            self.host, self.port, self.endpoint_path
        )
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

struct OpcuaInstance {
    handle: ServerHandle,
    server_task: JoinHandle<anyhow::Result<()>>,
    command_bridge_task: JoinHandle<()>,
    snapshot_sync_task: JoinHandle<()>,
}

impl OpcuaInstance {
    async fn stop(self) {
        self.handle.cancel();
        wait_for_task("server", self.server_task).await;
        wait_for_task("command bridge", self.command_bridge_task).await;
        wait_for_task("snapshot sync", self.snapshot_sync_task).await;
    }
}

impl OpcuaControl {
    pub async fn set_security_profile(&self, profile: impl Into<String>) -> anyhow::Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(OpcuaControlCommand::SetSecurityProfile {
                profile: profile.into(),
                response: response_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("OPC UA supervisor is not running"))?;
        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("OPC UA supervisor dropped response channel"))?
    }
}

#[derive(Debug, Clone)]
struct PendingCommand {
    source: CommandSourceEnum,
    sequence_id: u32,
    command_id: u16,
    param1: f64,
    param2: f64,
}

#[derive(Debug, Default)]
struct RequestShadow {
    sequence_id: u32,
    command_id: u16,
    param1: f64,
    param2: f64,
    last_execute: bool,
}

#[derive(Clone)]
struct CommandBranchNodes {
    rsp_ack_sequence: NodeId,
    rsp_status: NodeId,
    rsp_reject_reason: NodeId,
    rsp_last_update: NodeId,
}

#[derive(Clone)]
struct ProcedureNodes {
    state: NodeId,
    progress: NodeId,
    last_result: NodeId,
}

#[derive(Clone)]
struct OpcuaNodes {
    endpoint_url: NodeId,
    security_mode: NodeId,
    server_start_time_ms: NodeId,
    server_uptime_sec: NodeId,
    connected_clients: NodeId,
    connected_client_summary: NodeId,
    subscription_count: NodeId,
    publish_rate: NodeId,
    last_rejected: NodeId,
    last_error: NodeId,

    pressure_pa: NodeId,
    temperature_k: NodeId,
    o2_percent: NodeId,
    inner_door_pct: NodeId,
    outer_door_pct: NodeId,
    inner_door_closed: NodeId,
    outer_door_closed: NodeId,
    pump_on: NodeId,
    equalize_valve_pct: NodeId,
    vent_valve_pct: NodeId,
    alarm_summary: NodeId,
    out_of_spec: NodeId,

    operator_control_enabled: NodeId,
    remote_control_enabled: NodeId,
    leak_rate_nominal: NodeId,
    outer_unlock_max_pressure_pa: NodeId,
    inner_unlock_min_pressure_pa: NodeId,
    high_pressure_alarm_pa: NodeId,
    low_pressure_alarm_pa: NodeId,

    operation_mode: NodeId,
    source_mode: NodeId,
    command_en: NodeId,
    command_en_reason: NodeId,

    current_state: NodeId,
    time_in_state_sec: NodeId,
    transition_active: NodeId,
    blocking_condition: NodeId,

    active_procedure: NodeId,
    pea_tag_name: NodeId,
    health_state: NodeId,
    sim_time_sec: NodeId,
    fixed_timestep_sec: NodeId,
    fault_leak_rate_nominal: NodeId,

    vent_valve_element_position_pct: NodeId,
    equalize_valve_element_position_pct: NodeId,
    pump_drive_running: NodeId,
    pump_drive_current_a: NodeId,

    proc_depressurize: ProcedureNodes,
    proc_pressurize: ProcedureNodes,
    proc_manual_jog: ProcedureNodes,

    active_command: NodeId,
    active_source: NodeId,
    active_sequence: NodeId,
    active_param1: NodeId,
    active_param2: NodeId,
    active_state: NodeId,
    active_progress: NodeId,
    active_blocking: NodeId,
    active_start: NodeId,
    active_last_update: NodeId,

    operator: CommandBranchNodes,
    remote: CommandBranchNodes,
}

pub fn spawn_opcua_server(
    sim: Arc<RwLock<Simulation>>,
    snapshots_tx: broadcast::Sender<Snapshot>,
    config: OpcuaRuntimeConfig,
) -> OpcuaControl {
    let (control_tx, control_rx) = mpsc::channel(8);
    tokio::spawn(async move {
        run_opcua_supervisor(sim, snapshots_tx, control_rx, config).await;
    });
    OpcuaControl { tx: control_tx }
}

async fn run_opcua_supervisor(
    sim: Arc<RwLock<Simulation>>,
    snapshots_tx: broadcast::Sender<Snapshot>,
    mut control_rx: mpsc::Receiver<OpcuaControlCommand>,
    config: OpcuaRuntimeConfig,
) {
    let configured_profile = std::env::var("AIRLOCK_SECURITY_PROFILE")
        .unwrap_or_else(|_| "NONE".to_string())
        .trim()
        .to_string();
    let mut active_profile = match normalize_security_profile(&configured_profile) {
        Ok(profile) => profile,
        Err(err) => {
            warn!(
                "Invalid AIRLOCK_SECURITY_PROFILE={configured_profile}: {err}. Falling back to NONE"
            );
            "NONE".to_string()
        }
    };

    let mut instance =
        match start_opcua_instance(&config, &active_profile, sim.clone(), snapshots_tx.clone())
            .await
        {
            Ok(instance) => Some(instance),
            Err(err) => {
                error!("Initial OPC UA server startup failed: {err}");
                None
            }
        };

    while let Some(command) = control_rx.recv().await {
        match command {
            OpcuaControlCommand::SetSecurityProfile { profile, response } => {
                let normalized = match normalize_security_profile(&profile) {
                    Ok(normalized) => normalized,
                    Err(err) => {
                        let _ = response.send(Err(err));
                        continue;
                    }
                };

                if normalized == active_profile {
                    let _ = response.send(Ok(()));
                    continue;
                }

                if let Some(current) = instance.take() {
                    current.stop().await;
                }

                match start_opcua_instance(&config, &normalized, sim.clone(), snapshots_tx.clone())
                    .await
                {
                    Ok(next_instance) => {
                        info!(
                            "Switched OPC UA security profile from {} to {}",
                            active_profile, normalized
                        );
                        active_profile = normalized;
                        instance = Some(next_instance);
                        let _ = response.send(Ok(()));
                    }
                    Err(err) => {
                        error!("Failed to start OPC UA server with profile {normalized}: {err}");
                        match start_opcua_instance(
                            &config,
                            &active_profile,
                            sim.clone(),
                            snapshots_tx.clone(),
                        )
                        .await
                        {
                            Ok(previous_instance) => {
                                warn!("Rolled back OPC UA server to profile {}", active_profile);
                                instance = Some(previous_instance);
                            }
                            Err(rollback_err) => {
                                error!(
                                    "Failed to roll back OPC UA server to profile {}: {rollback_err}",
                                    active_profile
                                );
                                instance = None;
                            }
                        }
                        let _ = response.send(Err(anyhow::anyhow!(
                            "OPC UA restart failed for profile {normalized}: {err}"
                        )));
                    }
                }
            }
        }
    }

    if let Some(current) = instance {
        current.stop().await;
    }
}

fn normalize_security_profile(profile: &str) -> anyhow::Result<String> {
    let normalized = profile.trim().to_uppercase();
    if matches!(normalized.as_str(), "NONE" | "BASIC256SHA256" | "BOTH") {
        Ok(normalized)
    } else {
        Err(anyhow::anyhow!(
            "Unsupported security profile {profile}. Supported values: NONE | BASIC256SHA256 | BOTH"
        ))
    }
}

async fn start_opcua_instance(
    config: &OpcuaRuntimeConfig,
    security_profile: &str,
    sim: Arc<RwLock<Simulation>>,
    snapshots_tx: broadcast::Sender<Snapshot>,
) -> anyhow::Result<OpcuaInstance> {
    let user_tokens = vec![ANONYMOUS_USER_TOKEN_ID.to_string()];

    let endpoint_config_none = ServerEndpoint::new_none(config.endpoint_path.clone(), &user_tokens);
    let endpoint_config_none_root = ServerEndpoint::new_none("/".to_string(), &user_tokens);
    let endpoint_config_none_discovery =
        ServerEndpoint::new_none("/discovery".to_string(), &user_tokens);
    let endpoint_config_secure = ServerEndpoint::new(
        config.endpoint_path.clone(),
        SecurityPolicy::Basic256Sha256,
        MessageSecurityMode::SignAndEncrypt,
        &user_tokens,
    );
    let endpoint_config_secure_root = ServerEndpoint::new(
        "/".to_string(),
        SecurityPolicy::Basic256Sha256,
        MessageSecurityMode::SignAndEncrypt,
        &user_tokens,
    );
    let endpoint_config_secure_discovery = ServerEndpoint::new(
        "/discovery".to_string(),
        SecurityPolicy::Basic256Sha256,
        MessageSecurityMode::SignAndEncrypt,
        &user_tokens,
    );

    let base_builder = ServerBuilder::new()
        .application_name("Mars Airlock OPC UA Server")
        .application_uri("urn:mars-airlock:opcua-server")
        .product_uri("urn:mars-airlock")
        .create_sample_keypair(true)
        .host(config.host.clone())
        .port(config.port)
        .pki_dir("./pki")
        .trust_client_certs(true);

    let builder = match security_profile {
        "NONE" => base_builder
            .add_endpoint("none_main", endpoint_config_none)
            .add_endpoint("none_root", endpoint_config_none_root)
            .add_endpoint("none_discovery", endpoint_config_none_discovery),
        "BASIC256SHA256" => base_builder
            .add_endpoint("basic256sha256_sign_encrypt_main", endpoint_config_secure)
            .add_endpoint(
                "basic256sha256_sign_encrypt_root",
                endpoint_config_secure_root,
            )
            .add_endpoint(
                "basic256sha256_sign_encrypt_discovery",
                endpoint_config_secure_discovery,
            ),
        "BOTH" => base_builder
            .add_endpoint("none_main", endpoint_config_none)
            .add_endpoint("none_root", endpoint_config_none_root)
            .add_endpoint("none_discovery", endpoint_config_none_discovery)
            .add_endpoint("basic256sha256_sign_encrypt_main", endpoint_config_secure)
            .add_endpoint(
                "basic256sha256_sign_encrypt_root",
                endpoint_config_secure_root,
            )
            .add_endpoint(
                "basic256sha256_sign_encrypt_discovery",
                endpoint_config_secure_discovery,
            ),
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported security profile {security_profile}"
            ));
        }
    };

    let (server, handle) = builder
        .discovery_urls(vec![config.endpoint_path.clone()])
        .diagnostics_enabled(true)
        .with_node_manager(simple_node_manager(
            NamespaceMetadata {
                namespace_uri: NAMESPACE_URI.to_string(),
                ..Default::default()
            },
            "airlock",
        ))
        .build()
        .map_err(anyhow::Error::msg)?;

    let manager = handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("SimpleNodeManager not available"))?;

    let namespace_index = handle
        .get_namespace_index(NAMESPACE_URI)
        .ok_or_else(|| anyhow::anyhow!("Namespace index not available: {NAMESPACE_URI}"))?;

    let (nodes, command_rx) = build_address_space(namespace_index, &manager, security_profile)?;

    let command_bridge_task = spawn_command_bridge(
        command_rx,
        sim.clone(),
        snapshots_tx.clone(),
        handle.clone(),
    );
    let snapshot_sync_task = spawn_snapshot_sync(
        manager.clone(),
        handle.subscriptions().clone(),
        handle.clone(),
        nodes,
        sim.clone(),
    );

    let bind_host = config.bind_host.clone();
    let advertised_host = config.host.clone();
    let port = config.port;
    let endpoint_path = config.endpoint_path.clone();
    info!(
        "OPC UA server binding on {}:{} and advertising opc.tcp://{}:{}{}",
        bind_host, port, advertised_host, port, endpoint_path
    );

    let bind_addr = format!("{bind_host}:{port}");
    let use_default_listener = bind_host == advertised_host;
    let server_task = tokio::spawn(async move {
        if use_default_listener {
            server.run().await.map_err(anyhow::Error::msg)
        } else {
            let listener = TcpListener::bind(&bind_addr)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to bind OPC UA socket at {bind_addr}: {e}"))?;
            server.run_with(listener).await.map_err(anyhow::Error::msg)
        }
    });

    Ok(OpcuaInstance {
        handle,
        server_task,
        command_bridge_task,
        snapshot_sync_task,
    })
}

async fn wait_for_task<T>(name: &str, mut task: JoinHandle<T>) {
    match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
        Ok(_) => {}
        Err(_) => {
            warn!("Timed out waiting for OPC UA {name} task to stop; aborting");
            task.abort();
            let _ = task.await;
        }
    }
}

fn build_address_space(
    ns: u16,
    manager: &Arc<opcua::server::node_manager::memory::InMemoryNodeManager<SimpleNodeManagerImpl>>,
    startup_security_profile: &str,
) -> anyhow::Result<(OpcuaNodes, mpsc::UnboundedReceiver<PendingCommand>)> {
    let (tx, rx) = mpsc::unbounded_channel::<PendingCommand>();

    let mars_base = NodeId::new(ns, "MarsBase");
    let airlock_pea = NodeId::new(ns, "MarsBase.AirlockPEA");

    let diagnostics = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics");
    let simulation = NodeId::new(ns, "MarsBase.AirlockPEA.Simulation");
    let fault_injection = NodeId::new(ns, "MarsBase.AirlockPEA.FaultInjection");
    let service_set = NodeId::new(ns, "MarsBase.AirlockPEA.ServiceSet");
    let airlock_service = NodeId::new(ns, "MarsBase.AirlockPEA.ServiceSet.AirlockService");
    let service_info = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.ServiceInformation",
    );
    let modes = NodeId::new(ns, "MarsBase.AirlockPEA.ServiceSet.AirlockService.Modes");
    let state_machine = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.StateMachine",
    );
    let procedures = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures",
    );
    let data_assemblies = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies",
    );
    let indicators = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators",
    );
    let parameters = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters",
    );
    let active_elements = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements",
    );
    let control = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control",
    );
    let active_command = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand",
    );
    let operator_commands_path =
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.OperatorCommands";
    let remote_commands_path =
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.RemoteCommands";
    let operator_commands = NodeId::new(ns, operator_commands_path);
    let remote_commands = NodeId::new(ns, remote_commands_path);
    let proc_depressurize = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA",
    );
    let proc_pressurize = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry",
    );
    let proc_manual_jog = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog",
    );
    let proc_depressurize_parameters = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA.Parameters",
    );
    let proc_depressurize_status = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA.Status",
    );
    let proc_depressurize_report = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA.ReportValues",
    );
    let proc_pressurize_parameters = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry.Parameters",
    );
    let proc_pressurize_status = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry.Status",
    );
    let proc_pressurize_report = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry.ReportValues",
    );
    let proc_manual_jog_parameters = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog.Parameters",
    );
    let proc_manual_jog_status = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog.Status",
    );
    let proc_manual_jog_report = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog.ReportValues",
    );
    let vent_valve_element = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements.VentValve",
    );
    let equalize_valve_element = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements.EqualizeValve",
    );
    let pump_drive_element = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements.VacuumPumpDrive",
    );

    let mut address_space = manager.address_space().write();
    address_space.add_folder(
        &mars_base,
        "MarsBase",
        "MarsBase",
        &NodeId::objects_folder_id(),
    );
    address_space.add_folder(&airlock_pea, "AirlockPEA", "AirlockPEA", &mars_base);

    for (node_id, browse) in [
        (&diagnostics, "Diagnostics"),
        (&simulation, "Simulation"),
        (&fault_injection, "FaultInjection"),
        (&service_set, "ServiceSet"),
        (&airlock_service, "AirlockService"),
        (&service_info, "ServiceInformation"),
        (&modes, "Modes"),
        (&state_machine, "StateMachine"),
        (&procedures, "Procedures"),
        (&data_assemblies, "DataAssemblies"),
        (&indicators, "Indicators"),
        (&parameters, "Parameters"),
        (&active_elements, "ActiveElements"),
        (&control, "Control"),
        (&active_command, "ActiveCommand"),
        (&operator_commands, "OperatorCommands"),
        (&remote_commands, "RemoteCommands"),
        (&proc_depressurize, "Proc_DepressurizeForEVA"),
        (&proc_pressurize, "Proc_PressurizeForEntry"),
        (&proc_manual_jog, "Proc_ManualDoorJog"),
        (
            &proc_depressurize_parameters,
            "Proc_DepressurizeForEVA.Parameters",
        ),
        (&proc_depressurize_status, "Proc_DepressurizeForEVA.Status"),
        (
            &proc_depressurize_report,
            "Proc_DepressurizeForEVA.ReportValues",
        ),
        (
            &proc_pressurize_parameters,
            "Proc_PressurizeForEntry.Parameters",
        ),
        (&proc_pressurize_status, "Proc_PressurizeForEntry.Status"),
        (
            &proc_pressurize_report,
            "Proc_PressurizeForEntry.ReportValues",
        ),
        (&proc_manual_jog_parameters, "Proc_ManualDoorJog.Parameters"),
        (&proc_manual_jog_status, "Proc_ManualDoorJog.Status"),
        (&proc_manual_jog_report, "Proc_ManualDoorJog.ReportValues"),
        (&vent_valve_element, "VentValve"),
        (&equalize_valve_element, "EqualizeValve"),
        (&pump_drive_element, "VacuumPumpDrive"),
    ] {
        let parent = match browse {
            "Diagnostics" | "Simulation" | "FaultInjection" | "ServiceSet" => &airlock_pea,
            "AirlockService" => &service_set,
            "ServiceInformation" | "Modes" | "StateMachine" | "Procedures" | "DataAssemblies" => {
                &airlock_service
            }
            "Indicators" | "Parameters" | "ActiveElements" | "Control" => &data_assemblies,
            "ActiveCommand" | "OperatorCommands" | "RemoteCommands" => &control,
            "Proc_DepressurizeForEVA" | "Proc_PressurizeForEntry" | "Proc_ManualDoorJog" => {
                &procedures
            }
            "Proc_DepressurizeForEVA.Parameters"
            | "Proc_DepressurizeForEVA.Status"
            | "Proc_DepressurizeForEVA.ReportValues" => &proc_depressurize,
            "Proc_PressurizeForEntry.Parameters"
            | "Proc_PressurizeForEntry.Status"
            | "Proc_PressurizeForEntry.ReportValues" => &proc_pressurize,
            "Proc_ManualDoorJog.Parameters"
            | "Proc_ManualDoorJog.Status"
            | "Proc_ManualDoorJog.ReportValues" => &proc_manual_jog,
            "VentValve" | "EqualizeValve" | "VacuumPumpDrive" => &active_elements,
            _ => &airlock_pea,
        };
        address_space.add_folder(node_id, browse, browse, parent);
    }

    let endpoint_url = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.EndpointUrl");
    let security_mode = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.SecurityMode");
    let server_start_time_ms = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.ServerStartTimeMs");
    let server_uptime_sec = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.ServerUptimeSec");
    let connected_clients = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.ConnectedClientCount");
    let connected_client_summary =
        NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.ConnectedClientSummary");
    let subscription_count = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.SubscriptionCount");
    let publish_rate = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.PublishingRateHz");
    let last_rejected = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.LastRejectedCommand");
    let last_error = NodeId::new(ns, "MarsBase.AirlockPEA.Diagnostics.LastError");

    insert_var(
        &mut address_space,
        &diagnostics,
        &endpoint_url,
        "EndpointUrl",
        "",
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &security_mode,
        "SecurityMode",
        startup_security_profile,
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &server_start_time_ms,
        "ServerStartTimeMs",
        0u64,
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &server_uptime_sec,
        "ServerUptimeSec",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &connected_clients,
        "ConnectedClientCount",
        0u32,
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &connected_client_summary,
        "ConnectedClientSummary",
        "none",
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &subscription_count,
        "SubscriptionCount",
        0u32,
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &publish_rate,
        "PublishingRateHz",
        10.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &last_rejected,
        "LastRejectedCommand",
        "",
        false,
    );
    insert_var(
        &mut address_space,
        &diagnostics,
        &last_error,
        "LastError",
        "",
        false,
    );

    let sim_time_sec = NodeId::new(ns, "MarsBase.AirlockPEA.Simulation.SimTimeSec");
    let fixed_timestep_sec = NodeId::new(ns, "MarsBase.AirlockPEA.Simulation.FixedTimestepSec");
    let fault_leak_rate_nominal =
        NodeId::new(ns, "MarsBase.AirlockPEA.FaultInjection.LeakRateNominal");

    insert_var(
        &mut address_space,
        &simulation,
        &sim_time_sec,
        "SimTimeSec",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &simulation,
        &fixed_timestep_sec,
        "FixedTimestepSec",
        0.05f64,
        false,
    );
    insert_var(
        &mut address_space,
        &fault_injection,
        &fault_leak_rate_nominal,
        "LeakRateNominal",
        0.0005f64,
        false,
    );

    let pressure_pa = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.PressurePa",
    );
    let temperature_k = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.TemperatureK",
    );
    let o2_percent = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.O2Percent",
    );
    let inner_door_pct = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.InnerDoorPositionPct",
    );
    let outer_door_pct = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.OuterDoorPositionPct",
    );
    let inner_door_closed = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.InnerDoorClosed",
    );
    let outer_door_closed = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.OuterDoorClosed",
    );
    let pump_on = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.PumpOn",
    );
    let equalize_valve_pct = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.EqualizeValvePct",
    );
    let vent_valve_pct = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.VentValvePct",
    );
    let alarm_summary = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.AlarmSummary",
    );
    let out_of_spec = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Indicators.OutOfSpec",
    );

    insert_var(
        &mut address_space,
        &indicators,
        &pressure_pa,
        "PressurePa",
        101325.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &temperature_k,
        "TemperatureK",
        293.15f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &o2_percent,
        "O2Percent",
        21.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &inner_door_pct,
        "InnerDoorPositionPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &outer_door_pct,
        "OuterDoorPositionPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &inner_door_closed,
        "InnerDoorClosed",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &outer_door_closed,
        "OuterDoorClosed",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &pump_on,
        "PumpOn",
        false,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &equalize_valve_pct,
        "EqualizeValvePct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &vent_valve_pct,
        "VentValvePct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &alarm_summary,
        "AlarmSummary",
        "OK",
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &out_of_spec,
        "OutOfSpec",
        false,
        false,
    );

    let operator_control_enabled = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters.OperatorControlEnabled",
    );
    let remote_control_enabled = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters.RemoteControlEnabled",
    );
    let leak_rate_nominal = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters.LeakRateNominal",
    );
    let outer_unlock_max_pressure_pa = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters.OuterUnlockMaxPressurePa",
    );
    let inner_unlock_min_pressure_pa = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters.InnerUnlockMinPressurePa",
    );
    let high_pressure_alarm_pa = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters.HighPressureAlarmPa",
    );
    let low_pressure_alarm_pa = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Parameters.LowPressureAlarmPa",
    );

    insert_var(
        &mut address_space,
        &parameters,
        &operator_control_enabled,
        "OperatorControlEnabled",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &parameters,
        &remote_control_enabled,
        "RemoteControlEnabled",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &parameters,
        &leak_rate_nominal,
        "LeakRateNominal",
        0.0005f64,
        false,
    );
    insert_var(
        &mut address_space,
        &parameters,
        &outer_unlock_max_pressure_pa,
        "OuterUnlockMaxPressurePa",
        5000.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &parameters,
        &inner_unlock_min_pressure_pa,
        "InnerUnlockMinPressurePa",
        90000.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &parameters,
        &high_pressure_alarm_pa,
        "HighPressureAlarmPa",
        110000.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &parameters,
        &low_pressure_alarm_pa,
        "LowPressureAlarmPa",
        2000.0f64,
        false,
    );

    let operation_mode = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Modes.OperationMode",
    );
    let source_mode = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Modes.SourceMode",
    );
    let command_en = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Modes.CommandEn",
    );
    let command_en_reason = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Modes.CommandEnReason",
    );

    insert_var(
        &mut address_space,
        &modes,
        &operation_mode,
        "OperationMode",
        "AUTO",
        false,
    );
    insert_var(
        &mut address_space,
        &modes,
        &source_mode,
        "SourceMode",
        "SYSTEM_AUTO",
        false,
    );
    insert_var(
        &mut address_space,
        &modes,
        &command_en,
        "CommandEn",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &modes,
        &command_en_reason,
        "CommandEnReason",
        "",
        false,
    );

    let current_state = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.StateMachine.CurrentState",
    );
    let time_in_state_sec = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.StateMachine.TimeInStateSec",
    );
    let transition_active = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.StateMachine.TransitionActive",
    );
    let blocking_condition = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.StateMachine.BlockingCondition",
    );

    insert_var(
        &mut address_space,
        &state_machine,
        &current_state,
        "CurrentState",
        "idle",
        false,
    );
    insert_var(
        &mut address_space,
        &state_machine,
        &time_in_state_sec,
        "TimeInStateSec",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &state_machine,
        &transition_active,
        "TransitionActive",
        false,
        false,
    );
    insert_var(
        &mut address_space,
        &state_machine,
        &blocking_condition,
        "BlockingCondition",
        "",
        false,
    );

    let active_procedure = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.ServiceInformation.ActiveProcedure",
    );
    let pea_tag_name = NodeId::new(ns, "MarsBase.AirlockPEA.PEAInformationLabel.TagName");
    let health_state = NodeId::new(ns, "MarsBase.AirlockPEA.PEAInformationLabel.HealthState");
    let pea_info = NodeId::new(ns, "MarsBase.AirlockPEA.PEAInformationLabel");
    address_space.add_folder(
        &pea_info,
        "PEAInformationLabel",
        "PEAInformationLabel",
        &airlock_pea,
    );
    insert_var(
        &mut address_space,
        &service_info,
        &active_procedure,
        "ActiveProcedure",
        "None",
        false,
    );
    insert_var(
        &mut address_space,
        &pea_info,
        &pea_tag_name,
        "TagName",
        "AIRLOCK-PEA-001",
        false,
    );
    insert_var(
        &mut address_space,
        &pea_info,
        &health_state,
        "HealthState",
        "OK",
        false,
    );

    let proc_depressurize_target_pressure = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA.Parameters.TargetPressurePa",
    );
    let proc_depressurize_state = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA.Status.State",
    );
    let proc_depressurize_progress = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA.Status.ProgressPct",
    );
    let proc_depressurize_last_result = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_DepressurizeForEVA.ReportValues.LastResult",
    );
    insert_var(
        &mut address_space,
        &proc_depressurize_parameters,
        &proc_depressurize_target_pressure,
        "TargetPressurePa",
        5000.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &proc_depressurize_status,
        &proc_depressurize_state,
        "State",
        "idle",
        false,
    );
    insert_var(
        &mut address_space,
        &proc_depressurize_status,
        &proc_depressurize_progress,
        "ProgressPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &proc_depressurize_report,
        &proc_depressurize_last_result,
        "LastResult",
        "",
        false,
    );

    let proc_pressurize_target_pressure = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry.Parameters.TargetPressurePa",
    );
    let proc_pressurize_state = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry.Status.State",
    );
    let proc_pressurize_progress = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry.Status.ProgressPct",
    );
    let proc_pressurize_last_result = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_PressurizeForEntry.ReportValues.LastResult",
    );
    insert_var(
        &mut address_space,
        &proc_pressurize_parameters,
        &proc_pressurize_target_pressure,
        "TargetPressurePa",
        90000.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &proc_pressurize_status,
        &proc_pressurize_state,
        "State",
        "idle",
        false,
    );
    insert_var(
        &mut address_space,
        &proc_pressurize_status,
        &proc_pressurize_progress,
        "ProgressPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &proc_pressurize_report,
        &proc_pressurize_last_result,
        "LastResult",
        "",
        false,
    );

    let proc_manual_jog_target_pct = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog.Parameters.TargetDoorPct",
    );
    let proc_manual_jog_state = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog.Status.State",
    );
    let proc_manual_jog_progress = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog.Status.ProgressPct",
    );
    let proc_manual_jog_last_result = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.Procedures.Proc_ManualDoorJog.ReportValues.LastResult",
    );
    insert_var(
        &mut address_space,
        &proc_manual_jog_parameters,
        &proc_manual_jog_target_pct,
        "TargetDoorPct",
        100.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &proc_manual_jog_status,
        &proc_manual_jog_state,
        "State",
        "idle",
        false,
    );
    insert_var(
        &mut address_space,
        &proc_manual_jog_status,
        &proc_manual_jog_progress,
        "ProgressPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &proc_manual_jog_report,
        &proc_manual_jog_last_result,
        "LastResult",
        "",
        false,
    );

    let vent_valve_element_position_pct = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements.VentValve.PositionPct",
    );
    let equalize_valve_element_position_pct = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements.EqualizeValve.PositionPct",
    );
    let pump_drive_running = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements.VacuumPumpDrive.Running",
    );
    let pump_drive_current_a = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.ActiveElements.VacuumPumpDrive.CurrentA",
    );
    insert_var(
        &mut address_space,
        &vent_valve_element,
        &vent_valve_element_position_pct,
        "PositionPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &equalize_valve_element,
        &equalize_valve_element_position_pct,
        "PositionPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &pump_drive_element,
        &pump_drive_running,
        "Running",
        false,
        false,
    );
    insert_var(
        &mut address_space,
        &pump_drive_element,
        &pump_drive_current_a,
        "CurrentA",
        0.0f64,
        false,
    );

    let active_cmd_id = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.Command",
    );
    let active_source = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.Source",
    );
    let active_sequence = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.SequenceId",
    );
    let active_param1 = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.Param1",
    );
    let active_param2 = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.Param2",
    );
    let active_state = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.State",
    );
    let active_progress = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.ProgressPct",
    );
    let active_blocking = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.BlockingCondition",
    );
    let active_start = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.StartTimeMs",
    );
    let active_last_update = NodeId::new(
        ns,
        "MarsBase.AirlockPEA.ServiceSet.AirlockService.DataAssemblies.Control.ActiveCommand.LastUpdateTimeMs",
    );

    insert_var(
        &mut address_space,
        &active_command,
        &active_cmd_id,
        "Command",
        0u16,
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_source,
        "Source",
        "SYSTEM_AUTO",
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_sequence,
        "SequenceId",
        0u32,
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_param1,
        "Param1",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_param2,
        "Param2",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_state,
        "State",
        0u16,
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_progress,
        "ProgressPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_blocking,
        "BlockingCondition",
        "",
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_start,
        "StartTimeMs",
        0u64,
        false,
    );
    insert_var(
        &mut address_space,
        &active_command,
        &active_last_update,
        "LastUpdateTimeMs",
        0u64,
        false,
    );

    let operator_shadow = Arc::new(Mutex::new(RequestShadow::default()));
    let remote_shadow = Arc::new(Mutex::new(RequestShadow::default()));

    let operator = add_command_branch(
        ns,
        &mut address_space,
        &operator_commands,
        operator_commands_path,
        "Operator",
        CommandSourceEnum::OperatorUi,
        operator_shadow,
        tx.clone(),
        manager,
    );
    let remote = add_command_branch(
        ns,
        &mut address_space,
        &remote_commands,
        remote_commands_path,
        "Remote",
        CommandSourceEnum::RemoteOpcua,
        remote_shadow,
        tx,
        manager,
    );

    drop(address_space);

    Ok((
        OpcuaNodes {
            endpoint_url,
            security_mode,
            server_start_time_ms,
            server_uptime_sec,
            connected_clients,
            connected_client_summary,
            subscription_count,
            publish_rate,
            last_rejected,
            last_error,
            pressure_pa,
            temperature_k,
            o2_percent,
            inner_door_pct,
            outer_door_pct,
            inner_door_closed,
            outer_door_closed,
            pump_on,
            equalize_valve_pct,
            vent_valve_pct,
            alarm_summary,
            out_of_spec,
            operator_control_enabled,
            remote_control_enabled,
            leak_rate_nominal,
            outer_unlock_max_pressure_pa,
            inner_unlock_min_pressure_pa,
            high_pressure_alarm_pa,
            low_pressure_alarm_pa,
            operation_mode,
            source_mode,
            command_en,
            command_en_reason,
            current_state,
            time_in_state_sec,
            transition_active,
            blocking_condition,
            active_procedure,
            pea_tag_name,
            health_state,
            sim_time_sec,
            fixed_timestep_sec,
            fault_leak_rate_nominal,
            vent_valve_element_position_pct,
            equalize_valve_element_position_pct,
            pump_drive_running,
            pump_drive_current_a,
            proc_depressurize: ProcedureNodes {
                state: proc_depressurize_state,
                progress: proc_depressurize_progress,
                last_result: proc_depressurize_last_result,
            },
            proc_pressurize: ProcedureNodes {
                state: proc_pressurize_state,
                progress: proc_pressurize_progress,
                last_result: proc_pressurize_last_result,
            },
            proc_manual_jog: ProcedureNodes {
                state: proc_manual_jog_state,
                progress: proc_manual_jog_progress,
                last_result: proc_manual_jog_last_result,
            },
            active_command: active_cmd_id,
            active_source,
            active_sequence,
            active_param1,
            active_param2,
            active_state,
            active_progress,
            active_blocking,
            active_start,
            active_last_update,
            operator,
            remote,
        },
        rx,
    ))
}

#[allow(clippy::too_many_arguments)]
fn add_command_branch(
    ns: u16,
    address_space: &mut AddressSpace,
    parent: &NodeId,
    parent_path: &str,
    prefix: &str,
    source: CommandSourceEnum,
    shadow: Arc<Mutex<RequestShadow>>,
    tx: mpsc::UnboundedSender<PendingCommand>,
    manager: &Arc<opcua::server::node_manager::memory::InMemoryNodeManager<SimpleNodeManagerImpl>>,
) -> CommandBranchNodes {
    let req_folder = NodeId::new(ns, format!("{parent_path}.Req"));
    let rsp_folder = NodeId::new(ns, format!("{parent_path}.Rsp"));

    address_space.add_folder(&req_folder, "Req", "Req", parent);
    address_space.add_folder(&rsp_folder, "Rsp", "Rsp", parent);

    let req_sequence = NodeId::new(ns, format!("{parent_path}.Req.SequenceId"));
    let req_command = NodeId::new(ns, format!("{parent_path}.Req.Command"));
    let req_param1 = NodeId::new(ns, format!("{parent_path}.Req.Param1"));
    let req_param2 = NodeId::new(ns, format!("{parent_path}.Req.Param2"));
    let req_execute = NodeId::new(ns, format!("{parent_path}.Req.Execute"));

    insert_var(
        address_space,
        &req_folder,
        &req_sequence,
        "SequenceId",
        0u32,
        true,
    );
    insert_var(
        address_space,
        &req_folder,
        &req_command,
        "Command",
        0u16,
        true,
    );
    insert_var(
        address_space,
        &req_folder,
        &req_param1,
        "Param1",
        0.0f64,
        true,
    );
    insert_var(
        address_space,
        &req_folder,
        &req_param2,
        "Param2",
        0.0f64,
        true,
    );
    insert_var(
        address_space,
        &req_folder,
        &req_execute,
        "Execute",
        false,
        true,
    );

    let rsp_ack_sequence = NodeId::new(ns, format!("{parent_path}.Rsp.AckSequenceId"));
    let rsp_status = NodeId::new(ns, format!("{parent_path}.Rsp.Status"));
    let rsp_reject_reason = NodeId::new(ns, format!("{parent_path}.Rsp.RejectReason"));
    let rsp_last_update = NodeId::new(ns, format!("{parent_path}.Rsp.LastUpdateTimeMs"));

    insert_var(
        address_space,
        &rsp_folder,
        &rsp_ack_sequence,
        "AckSequenceId",
        0u32,
        false,
    );
    insert_var(
        address_space,
        &rsp_folder,
        &rsp_status,
        "Status",
        0u16,
        false,
    );
    insert_var(
        address_space,
        &rsp_folder,
        &rsp_reject_reason,
        "RejectReason",
        "",
        false,
    );
    insert_var(
        address_space,
        &rsp_folder,
        &rsp_last_update,
        "LastUpdateTimeMs",
        0u64,
        false,
    );

    let manager_inner = manager.inner();

    {
        let shadow = shadow.clone();
        manager_inner.add_write_callback(req_sequence.clone(), move |dv, _| {
            let Some(value) = dv.value.as_ref().and_then(parse_u32_variant) else {
                return StatusCode::BadTypeMismatch;
            };
            if let Ok(mut state) = shadow.lock() {
                state.sequence_id = value;
                StatusCode::Good
            } else {
                StatusCode::BadInternalError
            }
        });
    }

    {
        let shadow = shadow.clone();
        manager_inner.add_write_callback(req_command.clone(), move |dv, _| {
            let Some(value) = dv.value.as_ref().and_then(parse_u16_variant) else {
                return StatusCode::BadTypeMismatch;
            };
            if let Ok(mut state) = shadow.lock() {
                state.command_id = value;
                StatusCode::Good
            } else {
                StatusCode::BadInternalError
            }
        });
    }

    {
        let shadow = shadow.clone();
        manager_inner.add_write_callback(req_param1.clone(), move |dv, _| {
            let Some(value) = dv.value.as_ref().and_then(parse_f64_variant) else {
                return StatusCode::BadTypeMismatch;
            };
            if let Ok(mut state) = shadow.lock() {
                state.param1 = value;
                StatusCode::Good
            } else {
                StatusCode::BadInternalError
            }
        });
    }

    {
        let shadow = shadow.clone();
        manager_inner.add_write_callback(req_param2.clone(), move |dv, _| {
            let Some(value) = dv.value.as_ref().and_then(parse_f64_variant) else {
                return StatusCode::BadTypeMismatch;
            };
            if let Ok(mut state) = shadow.lock() {
                state.param2 = value;
                StatusCode::Good
            } else {
                StatusCode::BadInternalError
            }
        });
    }

    {
        let shadow = shadow.clone();
        manager_inner.add_write_callback(req_execute.clone(), move |dv, _| {
            let Some(execute) = dv.value.as_ref().and_then(parse_bool_variant) else {
                return StatusCode::BadTypeMismatch;
            };

            if let Ok(mut state) = shadow.lock() {
                if execute && !state.last_execute {
                    let pending = PendingCommand {
                        source,
                        sequence_id: state.sequence_id,
                        command_id: state.command_id,
                        param1: state.param1,
                        param2: state.param2,
                    };
                    if tx.send(pending).is_err() {
                        return StatusCode::BadUnexpectedError;
                    }
                }
                state.last_execute = execute;
                StatusCode::Good
            } else {
                StatusCode::BadInternalError
            }
        });
    }

    info!("Configured OPC UA command branch {prefix}");

    CommandBranchNodes {
        rsp_ack_sequence,
        rsp_status,
        rsp_reject_reason,
        rsp_last_update,
    }
}

fn insert_var(
    address_space: &mut AddressSpace,
    parent: &NodeId,
    id: &NodeId,
    browse_name: &str,
    value: impl Into<Variant>,
    writable: bool,
) {
    let mut variable = Variable::new(id, browse_name, browse_name, value);
    if writable {
        variable.set_writable(true);
        let mut user_access = variable.user_access_level();
        user_access.insert(AccessLevel::CURRENT_WRITE);
        variable.set_user_access_level(user_access);
    }
    let _ = address_space.add_variables(vec![variable], parent);
}

fn spawn_command_bridge(
    mut rx: mpsc::UnboundedReceiver<PendingCommand>,
    sim: Arc<RwLock<Simulation>>,
    snapshots_tx: broadcast::Sender<Snapshot>,
    server_handle: ServerHandle,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let pending = tokio::select! {
                _ = server_handle.token().cancelled() => {
                    info!("OPC UA command bridge stopped");
                    break;
                }
                pending = rx.recv() => pending
            };

            let Some(pending) = pending else {
                break;
            };

            let Some(command) = command_from_id(pending.command_id) else {
                warn!(
                    "Ignoring unknown command id from OPC UA: {}",
                    pending.command_id
                );
                continue;
            };

            let request_true = CommandRequestFields {
                sequence_id: pending.sequence_id,
                command,
                param1: pending.param1,
                param2: pending.param2,
                execute: true,
            };

            let request_false = CommandRequestFields {
                execute: false,
                ..request_true.clone()
            };

            let snapshot = {
                let mut simulation = sim.write().await;
                let _ = simulation.write_request(pending.source, request_true);
                let _ = simulation.write_request(pending.source, request_false);
                simulation.snapshot()
            };

            let _ = snapshots_tx.send(snapshot);
        }
    })
}

fn spawn_snapshot_sync(
    manager: Arc<opcua::server::node_manager::memory::InMemoryNodeManager<SimpleNodeManagerImpl>>,
    subscriptions: Arc<SubscriptionCache>,
    server_handle: ServerHandle,
    nodes: OpcuaNodes,
    sim: Arc<RwLock<Simulation>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = server_handle.token().cancelled() => {
                    info!("OPC UA snapshot sync stopped");
                    break;
                }
                _ = interval.tick() => {}
            }

            let server_start_time_ms = server_start_time_ms(&server_handle);
            let (opcua_sessions, opcua_subscriptions) =
                read_opcua_diagnostics_counts(&server_handle);
            let server_uptime_sec =
                (Simulation::now_ms().saturating_sub(server_start_time_ms)) as f64 / 1000.0;

            let snapshot = {
                let mut simulation = sim.write().await;
                simulation.set_opcua_diagnostics(
                    server_start_time_ms,
                    server_uptime_sec,
                    opcua_sessions,
                    opcua_subscriptions,
                );
                simulation.snapshot()
            };

            let proc_depressurize = procedure_runtime_by_name(&snapshot, "Proc_DepressurizeForEVA");
            let proc_pressurize = procedure_runtime_by_name(&snapshot, "Proc_PressurizeForEntry");
            let proc_manual_jog = procedure_runtime_by_name(&snapshot, "Proc_ManualDoorJog");

            if let Err(err) = manager.set_values(
                &subscriptions,
                [
                    (
                        &nodes.endpoint_url,
                        None,
                        DataValue::new_now(snapshot.diagnostics.endpoint_url.clone()),
                    ),
                    (
                        &nodes.security_mode,
                        None,
                        DataValue::new_now(snapshot.diagnostics.active_security_mode.clone()),
                    ),
                    (
                        &nodes.server_start_time_ms,
                        None,
                        DataValue::new_now(snapshot.diagnostics.server_start_time_ms),
                    ),
                    (
                        &nodes.server_uptime_sec,
                        None,
                        DataValue::new_now(snapshot.diagnostics.server_uptime_sec),
                    ),
                    (
                        &nodes.connected_clients,
                        None,
                        DataValue::new_now(snapshot.diagnostics.connected_client_count),
                    ),
                    (
                        &nodes.connected_client_summary,
                        None,
                        DataValue::new_now(snapshot.diagnostics.connected_client_summary.clone()),
                    ),
                    (
                        &nodes.subscription_count,
                        None,
                        DataValue::new_now(snapshot.diagnostics.subscription_count),
                    ),
                    (
                        &nodes.publish_rate,
                        None,
                        DataValue::new_now(snapshot.diagnostics.publishing_rate_hz),
                    ),
                    (
                        &nodes.last_rejected,
                        None,
                        DataValue::new_now(snapshot.diagnostics.last_rejected_command.clone()),
                    ),
                    (
                        &nodes.last_error,
                        None,
                        DataValue::new_now(snapshot.diagnostics.last_error.clone()),
                    ),
                    (
                        &nodes.pressure_pa,
                        None,
                        DataValue::new_now(snapshot.pressure_pa),
                    ),
                    (
                        &nodes.temperature_k,
                        None,
                        DataValue::new_now(snapshot.temperature_k),
                    ),
                    (
                        &nodes.o2_percent,
                        None,
                        DataValue::new_now(snapshot.o2_percent),
                    ),
                    (
                        &nodes.inner_door_pct,
                        None,
                        DataValue::new_now(snapshot.inner_door_position_pct),
                    ),
                    (
                        &nodes.outer_door_pct,
                        None,
                        DataValue::new_now(snapshot.outer_door_position_pct),
                    ),
                    (
                        &nodes.inner_door_closed,
                        None,
                        DataValue::new_now(snapshot.inner_door_position_pct <= 0.1),
                    ),
                    (
                        &nodes.outer_door_closed,
                        None,
                        DataValue::new_now(snapshot.outer_door_position_pct <= 0.1),
                    ),
                    (&nodes.pump_on, None, DataValue::new_now(snapshot.pump_on)),
                    (
                        &nodes.equalize_valve_pct,
                        None,
                        DataValue::new_now(snapshot.equalize_valve_pct),
                    ),
                    (
                        &nodes.vent_valve_pct,
                        None,
                        DataValue::new_now(snapshot.vent_valve_pct),
                    ),
                    (
                        &nodes.alarm_summary,
                        None,
                        DataValue::new_now(snapshot.alarms.alarm_summary.clone()),
                    ),
                    (
                        &nodes.out_of_spec,
                        None,
                        DataValue::new_now(snapshot.alarms.out_of_spec),
                    ),
                    (
                        &nodes.operator_control_enabled,
                        None,
                        DataValue::new_now(snapshot.permissions.operator_control_enabled),
                    ),
                    (
                        &nodes.remote_control_enabled,
                        None,
                        DataValue::new_now(snapshot.permissions.remote_control_enabled),
                    ),
                    (
                        &nodes.leak_rate_nominal,
                        None,
                        DataValue::new_now(snapshot.leak_rate_nominal),
                    ),
                    (
                        &nodes.outer_unlock_max_pressure_pa,
                        None,
                        DataValue::new_now(snapshot.outer_unlock_max_pressure_pa),
                    ),
                    (
                        &nodes.inner_unlock_min_pressure_pa,
                        None,
                        DataValue::new_now(snapshot.inner_unlock_min_pressure_pa),
                    ),
                    (
                        &nodes.high_pressure_alarm_pa,
                        None,
                        DataValue::new_now(snapshot.high_pressure_alarm_pa),
                    ),
                    (
                        &nodes.low_pressure_alarm_pa,
                        None,
                        DataValue::new_now(snapshot.low_pressure_alarm_pa),
                    ),
                    (
                        &nodes.operation_mode,
                        None,
                        DataValue::new_now(
                            format!("{:?}", snapshot.mtp_modes.operation_mode).to_uppercase(),
                        ),
                    ),
                    (
                        &nodes.source_mode,
                        None,
                        DataValue::new_now(snapshot.mtp_modes.source_mode.as_str()),
                    ),
                    (
                        &nodes.command_en,
                        None,
                        DataValue::new_now(snapshot.mtp_modes.command_en),
                    ),
                    (
                        &nodes.command_en_reason,
                        None,
                        DataValue::new_now(snapshot.mtp_modes.command_en_reason.clone()),
                    ),
                    (
                        &nodes.current_state,
                        None,
                        DataValue::new_now(
                            format!("{:?}", snapshot.mtp_state_machine.current_state)
                                .to_lowercase(),
                        ),
                    ),
                    (
                        &nodes.time_in_state_sec,
                        None,
                        DataValue::new_now(snapshot.mtp_state_machine.time_in_state_sec),
                    ),
                    (
                        &nodes.transition_active,
                        None,
                        DataValue::new_now(snapshot.mtp_state_machine.transition_active),
                    ),
                    (
                        &nodes.blocking_condition,
                        None,
                        DataValue::new_now(snapshot.mtp_state_machine.blocking_condition.clone()),
                    ),
                    (
                        &nodes.active_procedure,
                        None,
                        DataValue::new_now(
                            snapshot
                                .mtp_runtime
                                .service_information
                                .active_procedure
                                .clone(),
                        ),
                    ),
                    (
                        &nodes.pea_tag_name,
                        None,
                        DataValue::new_now(
                            snapshot.mtp_runtime.pea_information_label.tag_name.clone(),
                        ),
                    ),
                    (
                        &nodes.health_state,
                        None,
                        DataValue::new_now(
                            snapshot
                                .mtp_runtime
                                .pea_information_label
                                .health_state
                                .clone(),
                        ),
                    ),
                    (
                        &nodes.sim_time_sec,
                        None,
                        DataValue::new_now(snapshot.sim_time_sec),
                    ),
                    (
                        &nodes.fixed_timestep_sec,
                        None,
                        DataValue::new_now(snapshot.fixed_timestep_sec),
                    ),
                    (
                        &nodes.fault_leak_rate_nominal,
                        None,
                        DataValue::new_now(snapshot.leak_rate_nominal),
                    ),
                    (
                        &nodes.vent_valve_element_position_pct,
                        None,
                        DataValue::new_now(snapshot.vent_valve_pct),
                    ),
                    (
                        &nodes.equalize_valve_element_position_pct,
                        None,
                        DataValue::new_now(snapshot.equalize_valve_pct),
                    ),
                    (
                        &nodes.pump_drive_running,
                        None,
                        DataValue::new_now(snapshot.pump_on),
                    ),
                    (
                        &nodes.pump_drive_current_a,
                        None,
                        DataValue::new_now(snapshot.pump_current_a),
                    ),
                    (
                        &nodes.proc_depressurize.state,
                        None,
                        DataValue::new_now(
                            proc_depressurize
                                .map(|entry| entry.proc_state.clone())
                                .unwrap_or_else(|| "idle".to_string()),
                        ),
                    ),
                    (
                        &nodes.proc_depressurize.progress,
                        None,
                        DataValue::new_now(
                            proc_depressurize
                                .map(|entry| entry.progress_pct)
                                .unwrap_or(0.0),
                        ),
                    ),
                    (
                        &nodes.proc_depressurize.last_result,
                        None,
                        DataValue::new_now(
                            proc_depressurize
                                .map(|entry| entry.last_result.clone())
                                .unwrap_or_default(),
                        ),
                    ),
                    (
                        &nodes.proc_pressurize.state,
                        None,
                        DataValue::new_now(
                            proc_pressurize
                                .map(|entry| entry.proc_state.clone())
                                .unwrap_or_else(|| "idle".to_string()),
                        ),
                    ),
                    (
                        &nodes.proc_pressurize.progress,
                        None,
                        DataValue::new_now(
                            proc_pressurize
                                .map(|entry| entry.progress_pct)
                                .unwrap_or(0.0),
                        ),
                    ),
                    (
                        &nodes.proc_pressurize.last_result,
                        None,
                        DataValue::new_now(
                            proc_pressurize
                                .map(|entry| entry.last_result.clone())
                                .unwrap_or_default(),
                        ),
                    ),
                    (
                        &nodes.proc_manual_jog.state,
                        None,
                        DataValue::new_now(
                            proc_manual_jog
                                .map(|entry| entry.proc_state.clone())
                                .unwrap_or_else(|| "idle".to_string()),
                        ),
                    ),
                    (
                        &nodes.proc_manual_jog.progress,
                        None,
                        DataValue::new_now(
                            proc_manual_jog
                                .map(|entry| entry.progress_pct)
                                .unwrap_or(0.0),
                        ),
                    ),
                    (
                        &nodes.proc_manual_jog.last_result,
                        None,
                        DataValue::new_now(
                            proc_manual_jog
                                .map(|entry| entry.last_result.clone())
                                .unwrap_or_default(),
                        ),
                    ),
                    (
                        &nodes.active_command,
                        None,
                        DataValue::new_now(command_to_id(snapshot.active_command.command)),
                    ),
                    (
                        &nodes.active_source,
                        None,
                        DataValue::new_now(snapshot.active_command.source.as_str()),
                    ),
                    (
                        &nodes.active_sequence,
                        None,
                        DataValue::new_now(snapshot.active_command.sequence_id),
                    ),
                    (
                        &nodes.active_param1,
                        None,
                        DataValue::new_now(snapshot.active_command.param1),
                    ),
                    (
                        &nodes.active_param2,
                        None,
                        DataValue::new_now(snapshot.active_command.param2),
                    ),
                    (
                        &nodes.active_state,
                        None,
                        DataValue::new_now(status_to_id(snapshot.active_command.state)),
                    ),
                    (
                        &nodes.active_progress,
                        None,
                        DataValue::new_now(snapshot.active_command.progress_pct),
                    ),
                    (
                        &nodes.active_blocking,
                        None,
                        DataValue::new_now(snapshot.active_command.blocking_condition.clone()),
                    ),
                    (
                        &nodes.active_start,
                        None,
                        DataValue::new_now(snapshot.active_command.start_time_ms),
                    ),
                    (
                        &nodes.active_last_update,
                        None,
                        DataValue::new_now(snapshot.active_command.last_update_time_ms),
                    ),
                    (
                        &nodes.operator.rsp_ack_sequence,
                        None,
                        DataValue::new_now(snapshot.operator_channel.rsp.ack_sequence_id),
                    ),
                    (
                        &nodes.operator.rsp_status,
                        None,
                        DataValue::new_now(status_to_id(snapshot.operator_channel.rsp.status)),
                    ),
                    (
                        &nodes.operator.rsp_reject_reason,
                        None,
                        DataValue::new_now(snapshot.operator_channel.rsp.reject_reason.clone()),
                    ),
                    (
                        &nodes.operator.rsp_last_update,
                        None,
                        DataValue::new_now(snapshot.operator_channel.rsp.last_update_time_ms),
                    ),
                    (
                        &nodes.remote.rsp_ack_sequence,
                        None,
                        DataValue::new_now(snapshot.remote_channel.rsp.ack_sequence_id),
                    ),
                    (
                        &nodes.remote.rsp_status,
                        None,
                        DataValue::new_now(status_to_id(snapshot.remote_channel.rsp.status)),
                    ),
                    (
                        &nodes.remote.rsp_reject_reason,
                        None,
                        DataValue::new_now(snapshot.remote_channel.rsp.reject_reason.clone()),
                    ),
                    (
                        &nodes.remote.rsp_last_update,
                        None,
                        DataValue::new_now(snapshot.remote_channel.rsp.last_update_time_ms),
                    ),
                ]
                .into_iter(),
            ) {
                warn!("Failed to sync OPC UA values: {err:?}");
            }
        }
    })
}

fn server_start_time_ms(server_handle: &ServerHandle) -> u64 {
    let start = server_handle.info().start_time.load();
    let millis = start.as_ref().as_chrono().timestamp_millis();
    if millis < 0 { 0 } else { millis as u64 }
}

fn read_opcua_diagnostics_counts(server_handle: &ServerHandle) -> (u32, u32) {
    let session_count = read_opcua_diagnostics_u32(
        server_handle,
        VariableId::Server_ServerDiagnostics_ServerDiagnosticsSummary_CurrentSessionCount,
    );
    let subscription_count = read_opcua_diagnostics_u32(
        server_handle,
        VariableId::Server_ServerDiagnostics_ServerDiagnosticsSummary_CurrentSubscriptionCount,
    );
    (session_count, subscription_count)
}

fn read_opcua_diagnostics_u32(server_handle: &ServerHandle, variable_id: VariableId) -> u32 {
    server_handle
        .info()
        .diagnostics
        .get(variable_id)
        .and_then(|value| value.value.as_ref().and_then(parse_u32_variant))
        .unwrap_or(0)
}

fn procedure_runtime_by_name<'a>(
    snapshot: &'a Snapshot,
    name: &str,
) -> Option<&'a ProcedureRuntime> {
    snapshot
        .mtp_runtime
        .procedures
        .iter()
        .find(|entry| entry.name == name)
}

fn parse_bool_variant(value: &Variant) -> Option<bool> {
    match value {
        Variant::Boolean(v) => Some(*v),
        Variant::Byte(v) => Some(*v != 0),
        Variant::SByte(v) => Some(*v != 0),
        Variant::UInt16(v) => Some(*v != 0),
        Variant::Int16(v) => Some(*v != 0),
        Variant::UInt32(v) => Some(*v != 0),
        Variant::Int32(v) => Some(*v != 0),
        _ => None,
    }
}

fn parse_u32_variant(value: &Variant) -> Option<u32> {
    match value {
        Variant::UInt32(v) => Some(*v),
        Variant::UInt16(v) => Some(*v as u32),
        Variant::Int32(v) if *v >= 0 => Some(*v as u32),
        Variant::Int16(v) if *v >= 0 => Some(*v as u32),
        Variant::Byte(v) => Some(*v as u32),
        _ => None,
    }
}

fn parse_u16_variant(value: &Variant) -> Option<u16> {
    match value {
        Variant::UInt16(v) => Some(*v),
        Variant::UInt32(v) if *v <= u16::MAX as u32 => Some(*v as u16),
        Variant::Int32(v) if *v >= 0 && *v <= u16::MAX as i32 => Some(*v as u16),
        Variant::Byte(v) => Some(*v as u16),
        _ => None,
    }
}

fn parse_f64_variant(value: &Variant) -> Option<f64> {
    match value {
        Variant::Double(v) => Some(*v),
        Variant::Float(v) => Some(*v as f64),
        Variant::Int32(v) => Some(*v as f64),
        Variant::UInt32(v) => Some(*v as f64),
        Variant::Int16(v) => Some(*v as f64),
        Variant::UInt16(v) => Some(*v as f64),
        Variant::Byte(v) => Some(*v as f64),
        _ => None,
    }
}

fn command_from_id(id: u16) -> Option<CommandEnum> {
    match id {
        0 => Some(CommandEnum::None),
        1 => Some(CommandEnum::StartDepressurizeCycle),
        2 => Some(CommandEnum::StartPressurizeCycle),
        3 => Some(CommandEnum::AbortCycle),
        4 => Some(CommandEnum::ResetFaults),
        5 => Some(CommandEnum::SetPumpOn),
        6 => Some(CommandEnum::SetEqualizeValvePct),
        7 => Some(CommandEnum::SetVentValvePct),
        8 => Some(CommandEnum::SetInnerDoorTargetPct),
        9 => Some(CommandEnum::SetOuterDoorTargetPct),
        10 => Some(CommandEnum::LockInnerDoor),
        11 => Some(CommandEnum::UnlockInnerDoor),
        12 => Some(CommandEnum::LockOuterDoor),
        13 => Some(CommandEnum::UnlockOuterDoor),
        _ => None,
    }
}

fn command_to_id(command: CommandEnum) -> u16 {
    match command {
        CommandEnum::None => 0,
        CommandEnum::StartDepressurizeCycle => 1,
        CommandEnum::StartPressurizeCycle => 2,
        CommandEnum::AbortCycle => 3,
        CommandEnum::ResetFaults => 4,
        CommandEnum::SetPumpOn => 5,
        CommandEnum::SetEqualizeValvePct => 6,
        CommandEnum::SetVentValvePct => 7,
        CommandEnum::SetInnerDoorTargetPct => 8,
        CommandEnum::SetOuterDoorTargetPct => 9,
        CommandEnum::LockInnerDoor => 10,
        CommandEnum::UnlockInnerDoor => 11,
        CommandEnum::LockOuterDoor => 12,
        CommandEnum::UnlockOuterDoor => 13,
    }
}

fn status_to_id(status: CommandStatusEnum) -> u16 {
    match status {
        CommandStatusEnum::Idle => 0,
        CommandStatusEnum::Accepted => 1,
        CommandStatusEnum::Rejected => 2,
        CommandStatusEnum::Running => 3,
        CommandStatusEnum::Complete => 4,
        CommandStatusEnum::Aborted => 5,
    }
}
