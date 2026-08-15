use std::{sync::Arc, time::Duration};

use opcua::{
    crypto::SecurityPolicy,
    server::{
        ANONYMOUS_USER_TOKEN_ID, ServerBuilder, ServerEndpoint,
        address_space::{AccessLevel, AddressSpace, Variable},
        diagnostics::NamespaceMetadata,
        node_manager::memory::{SimpleNodeManager, SimpleNodeManagerImpl, simple_node_manager},
    },
    types::{DataValue, MessageSecurityMode, NodeId, Variant},
};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{error, info, warn};

use crate::{
    PeaRuntimeState,
    subsystems::{EclssSimulation, PowerSimulation, SabatierSimulation, ThermalSimulation},
};

#[derive(Clone, Debug)]
struct SubsystemOpcuaConfig {
    bind_host: String,
    host: String,
    port: u16,
    endpoint_path: String,
    pki_dir: String,
    security_profile: String,
    application_name: String,
    application_uri: String,
}

impl SubsystemOpcuaConfig {
    fn new(
        port: u16,
        endpoint_path: &str,
        pki_dir: &str,
        security_profile: String,
        application_name: &str,
        application_uri: &str,
    ) -> Self {
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
            port,
            endpoint_path: endpoint_path.to_string(),
            pki_dir: pki_dir.to_string(),
            security_profile: security_profile.trim().to_uppercase(),
            application_name: application_name.to_string(),
            application_uri: application_uri.to_string(),
        }
    }

    fn endpoint_url(&self) -> String {
        format!(
            "opc.tcp://{}:{}{}",
            self.host, self.port, self.endpoint_path
        )
    }
}

#[derive(Clone)]
struct EclssNodes {
    endpoint_url: NodeId,
    security_mode: NodeId,
    service_state: NodeId,
    deployed: NodeId,
    running: NodeId,
    o2_percent: NodeId,
    co2_ppm: NodeId,
    humidity_pct: NodeId,
    water_recovery_pct: NodeId,
    power_kw: NodeId,
    alarm_high_co2: NodeId,
    alarm_low_o2: NodeId,
}

#[derive(Clone)]
struct SabatierNodes {
    endpoint_url: NodeId,
    security_mode: NodeId,
    service_state: NodeId,
    deployed: NodeId,
    running: NodeId,
    reactor_temp_c: NodeId,
    reactor_pressure_bar: NodeId,
    methane_production_kgph: NodeId,
    water_production_kgph: NodeId,
    conversion_efficiency_pct: NodeId,
    catalyst_health_pct: NodeId,
    power_kw: NodeId,
    alarm_reactor_temp: NodeId,
}

#[derive(Clone)]
struct PowerNodes {
    endpoint_url: NodeId,
    security_mode: NodeId,
    service_state: NodeId,
    deployed: NodeId,
    running: NodeId,
    generation_kw: NodeId,
    requested_load_kw: NodeId,
    served_load_kw: NodeId,
    battery_power_kw: NodeId,
    battery_energy_kwh: NodeId,
    battery_soc_pct: NodeId,
    bus_voltage_v: NodeId,
    unmet_load_kw: NodeId,
    load_shed_active: NodeId,
    alarm_battery_low: NodeId,
    alarm_bus_undervoltage: NodeId,
    balance_error_kw: NodeId,
}

#[derive(Clone)]
struct ThermalNodes {
    endpoint_url: NodeId,
    security_mode: NodeId,
    service_state: NodeId,
    deployed: NodeId,
    running: NodeId,
    ambient_temp_c: NodeId,
    habitat_temp_c: NodeId,
    coolant_supply_temp_c: NodeId,
    coolant_return_temp_c: NodeId,
    coolant_flow_kg_s: NodeId,
    radiator_deployment_pct: NodeId,
    heat_load_kw: NodeId,
    heat_rejection_kw: NodeId,
    pump_power_kw: NodeId,
    balance_error_kw: NodeId,
    cooling_available: NodeId,
    alarm_habitat_hot: NodeId,
    alarm_habitat_cold: NodeId,
    alarm_coolant_hot: NodeId,
}

pub fn spawn_eclss_opcua_server(
    sim: Arc<RwLock<EclssSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    port: u16,
    security_profile: String,
) {
    let config = SubsystemOpcuaConfig::new(
        port,
        "/underhill/eclss",
        "./pki/eclss",
        security_profile,
        "Underhill ECLSS OPC UA Server",
        "urn:underhill:eclss:opcua-server",
    );
    tokio::spawn(async move {
        if let Err(err) = run_eclss_opcua_server(sim, runtime, config).await {
            error!("ECLSS OPC UA server exited with error: {err}");
        }
    });
}

pub fn spawn_sabatier_opcua_server(
    sim: Arc<RwLock<SabatierSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    port: u16,
    security_profile: String,
) {
    let config = SubsystemOpcuaConfig::new(
        port,
        "/underhill/sabatier",
        "./pki/sabatier",
        security_profile,
        "Underhill Sabatier OPC UA Server",
        "urn:underhill:sabatier:opcua-server",
    );
    tokio::spawn(async move {
        if let Err(err) = run_sabatier_opcua_server(sim, runtime, config).await {
            error!("Sabatier OPC UA server exited with error: {err}");
        }
    });
}

pub fn spawn_power_opcua_server(
    sim: Arc<RwLock<PowerSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    port: u16,
    security_profile: String,
) {
    let config = SubsystemOpcuaConfig::new(
        port,
        "/underhill/power",
        "./pki/power",
        security_profile,
        "Underhill Power OPC UA Server",
        "urn:underhill:power:opcua-server",
    );
    tokio::spawn(async move {
        if let Err(err) = run_power_opcua_server(sim, runtime, config).await {
            error!("Power OPC UA server exited with error: {err}");
        }
    });
}

pub fn spawn_thermal_opcua_server(
    sim: Arc<RwLock<ThermalSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    port: u16,
    security_profile: String,
) {
    let config = SubsystemOpcuaConfig::new(
        port,
        "/underhill/thermal",
        "./pki/thermal",
        security_profile,
        "Underhill Thermal OPC UA Server",
        "urn:underhill:thermal:opcua-server",
    );
    tokio::spawn(async move {
        if let Err(err) = run_thermal_opcua_server(sim, runtime, config).await {
            error!("Thermal OPC UA server exited with error: {err}");
        }
    });
}

async fn run_eclss_opcua_server(
    sim: Arc<RwLock<EclssSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    config: SubsystemOpcuaConfig,
) -> anyhow::Result<()> {
    let namespace_uri = "urn:underhill:eclss:mtp";
    let (server, handle) = build_server(&config, namespace_uri)?;
    let manager = handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("SimpleNodeManager not available for ECLSS"))?;
    let ns = handle
        .get_namespace_index(namespace_uri)
        .ok_or_else(|| anyhow::anyhow!("Namespace index unavailable for ECLSS"))?;
    let nodes = build_eclss_address_space(ns, &manager);
    let subscriptions = handle.subscriptions().clone();
    let server_handle = handle.clone();
    let endpoint_url = config.endpoint_url();
    let security_profile = config.security_profile.clone();

    let sync_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = server_handle.token().cancelled() => {
                    info!("ECLSS OPC UA snapshot sync stopped");
                    break;
                }
                _ = interval.tick() => {}
            }

            let snapshot = sim.read().await.snapshot();
            let runtime_state = *runtime.read().await;
            let service_state = state_for_runtime(runtime_state);
            if let Err(err) = manager.set_values(
                &subscriptions,
                vec![
                    (
                        &nodes.endpoint_url,
                        None,
                        DataValue::new_now(endpoint_url.clone()),
                    ),
                    (
                        &nodes.security_mode,
                        None,
                        DataValue::new_now(security_profile.clone()),
                    ),
                    (
                        &nodes.service_state,
                        None,
                        DataValue::new_now(service_state),
                    ),
                    (
                        &nodes.deployed,
                        None,
                        DataValue::new_now(runtime_state.deployed),
                    ),
                    (
                        &nodes.running,
                        None,
                        DataValue::new_now(runtime_state.running),
                    ),
                    (
                        &nodes.o2_percent,
                        None,
                        DataValue::new_now(snapshot.o2_percent),
                    ),
                    (&nodes.co2_ppm, None, DataValue::new_now(snapshot.co2_ppm)),
                    (
                        &nodes.humidity_pct,
                        None,
                        DataValue::new_now(snapshot.humidity_pct),
                    ),
                    (
                        &nodes.water_recovery_pct,
                        None,
                        DataValue::new_now(snapshot.water_recovery_pct),
                    ),
                    (&nodes.power_kw, None, DataValue::new_now(snapshot.power_kw)),
                    (
                        &nodes.alarm_high_co2,
                        None,
                        DataValue::new_now(snapshot.alarm_high_co2),
                    ),
                    (
                        &nodes.alarm_low_o2,
                        None,
                        DataValue::new_now(snapshot.alarm_low_o2),
                    ),
                ]
                .into_iter(),
            ) {
                warn!("Failed updating ECLSS OPC UA values: {err}");
            }
        }
    });

    run_server(server, &config).await?;
    handle.cancel();
    let _ = sync_task.await;
    Ok(())
}

async fn run_sabatier_opcua_server(
    sim: Arc<RwLock<SabatierSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    config: SubsystemOpcuaConfig,
) -> anyhow::Result<()> {
    let namespace_uri = "urn:underhill:sabatier:mtp";
    let (server, handle) = build_server(&config, namespace_uri)?;
    let manager = handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("SimpleNodeManager not available for Sabatier"))?;
    let ns = handle
        .get_namespace_index(namespace_uri)
        .ok_or_else(|| anyhow::anyhow!("Namespace index unavailable for Sabatier"))?;
    let nodes = build_sabatier_address_space(ns, &manager);
    let subscriptions = handle.subscriptions().clone();
    let server_handle = handle.clone();
    let endpoint_url = config.endpoint_url();
    let security_profile = config.security_profile.clone();

    let sync_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = server_handle.token().cancelled() => {
                    info!("Sabatier OPC UA snapshot sync stopped");
                    break;
                }
                _ = interval.tick() => {}
            }

            let snapshot = sim.read().await.snapshot();
            let runtime_state = *runtime.read().await;
            let service_state = state_for_runtime(runtime_state);
            if let Err(err) = manager.set_values(
                &subscriptions,
                vec![
                    (
                        &nodes.endpoint_url,
                        None,
                        DataValue::new_now(endpoint_url.clone()),
                    ),
                    (
                        &nodes.security_mode,
                        None,
                        DataValue::new_now(security_profile.clone()),
                    ),
                    (
                        &nodes.service_state,
                        None,
                        DataValue::new_now(service_state),
                    ),
                    (
                        &nodes.deployed,
                        None,
                        DataValue::new_now(runtime_state.deployed),
                    ),
                    (
                        &nodes.running,
                        None,
                        DataValue::new_now(runtime_state.running),
                    ),
                    (
                        &nodes.reactor_temp_c,
                        None,
                        DataValue::new_now(snapshot.reactor_temp_c),
                    ),
                    (
                        &nodes.reactor_pressure_bar,
                        None,
                        DataValue::new_now(snapshot.reactor_pressure_bar),
                    ),
                    (
                        &nodes.methane_production_kgph,
                        None,
                        DataValue::new_now(snapshot.methane_production_kgph),
                    ),
                    (
                        &nodes.water_production_kgph,
                        None,
                        DataValue::new_now(snapshot.water_production_kgph),
                    ),
                    (
                        &nodes.conversion_efficiency_pct,
                        None,
                        DataValue::new_now(snapshot.conversion_efficiency_pct),
                    ),
                    (
                        &nodes.catalyst_health_pct,
                        None,
                        DataValue::new_now(snapshot.catalyst_health_pct),
                    ),
                    (&nodes.power_kw, None, DataValue::new_now(snapshot.power_kw)),
                    (
                        &nodes.alarm_reactor_temp,
                        None,
                        DataValue::new_now(snapshot.alarm_reactor_temp),
                    ),
                ]
                .into_iter(),
            ) {
                warn!("Failed updating Sabatier OPC UA values: {err}");
            }
        }
    });

    run_server(server, &config).await?;
    handle.cancel();
    let _ = sync_task.await;
    Ok(())
}

async fn run_power_opcua_server(
    sim: Arc<RwLock<PowerSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    config: SubsystemOpcuaConfig,
) -> anyhow::Result<()> {
    let namespace_uri = "urn:underhill:power:mtp";
    let (server, handle) = build_server(&config, namespace_uri)?;
    let manager = handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("SimpleNodeManager not available for Power"))?;
    let ns = handle
        .get_namespace_index(namespace_uri)
        .ok_or_else(|| anyhow::anyhow!("Namespace index unavailable for Power"))?;
    let nodes = build_power_address_space(ns, &manager);
    let subscriptions = handle.subscriptions().clone();
    let server_handle = handle.clone();
    let endpoint_url = config.endpoint_url();
    let security_profile = config.security_profile.clone();

    let sync_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = server_handle.token().cancelled() => break,
                _ = interval.tick() => {}
            }
            let snapshot = sim.read().await.snapshot();
            let runtime_state = *runtime.read().await;
            let values = vec![
                (
                    &nodes.endpoint_url,
                    DataValue::new_now(endpoint_url.clone()),
                ),
                (
                    &nodes.security_mode,
                    DataValue::new_now(security_profile.clone()),
                ),
                (
                    &nodes.service_state,
                    DataValue::new_now(state_for_runtime(runtime_state)),
                ),
                (&nodes.deployed, DataValue::new_now(runtime_state.deployed)),
                (&nodes.running, DataValue::new_now(runtime_state.running)),
                (
                    &nodes.generation_kw,
                    DataValue::new_now(snapshot.generation_kw),
                ),
                (
                    &nodes.requested_load_kw,
                    DataValue::new_now(snapshot.requested_load_kw),
                ),
                (
                    &nodes.served_load_kw,
                    DataValue::new_now(snapshot.served_load_kw),
                ),
                (
                    &nodes.battery_power_kw,
                    DataValue::new_now(snapshot.battery_power_kw),
                ),
                (
                    &nodes.battery_energy_kwh,
                    DataValue::new_now(snapshot.battery_energy_kwh),
                ),
                (
                    &nodes.battery_soc_pct,
                    DataValue::new_now(snapshot.battery_soc_pct),
                ),
                (
                    &nodes.bus_voltage_v,
                    DataValue::new_now(snapshot.bus_voltage_v),
                ),
                (
                    &nodes.unmet_load_kw,
                    DataValue::new_now(snapshot.unmet_load_kw),
                ),
                (
                    &nodes.load_shed_active,
                    DataValue::new_now(snapshot.load_shed_active),
                ),
                (
                    &nodes.alarm_battery_low,
                    DataValue::new_now(snapshot.alarm_battery_low),
                ),
                (
                    &nodes.alarm_bus_undervoltage,
                    DataValue::new_now(snapshot.alarm_bus_undervoltage),
                ),
                (
                    &nodes.balance_error_kw,
                    DataValue::new_now(snapshot.instantaneous_balance_error_kw),
                ),
            ];
            if let Err(err) = manager.set_values(
                &subscriptions,
                values.into_iter().map(|(node, value)| (node, None, value)),
            ) {
                warn!("Failed updating Power OPC UA values: {err}");
            }
        }
    });

    run_server(server, &config).await?;
    handle.cancel();
    let _ = sync_task.await;
    Ok(())
}

async fn run_thermal_opcua_server(
    sim: Arc<RwLock<ThermalSimulation>>,
    runtime: Arc<RwLock<PeaRuntimeState>>,
    config: SubsystemOpcuaConfig,
) -> anyhow::Result<()> {
    let namespace_uri = "urn:underhill:thermal:mtp";
    let (server, handle) = build_server(&config, namespace_uri)?;
    let manager = handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("SimpleNodeManager not available for Thermal"))?;
    let ns = handle
        .get_namespace_index(namespace_uri)
        .ok_or_else(|| anyhow::anyhow!("Namespace index unavailable for Thermal"))?;
    let nodes = build_thermal_address_space(ns, &manager);
    let subscriptions = handle.subscriptions().clone();
    let server_handle = handle.clone();
    let endpoint_url = config.endpoint_url();
    let security_profile = config.security_profile.clone();
    let sync_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = server_handle.token().cancelled() => break,
                _ = interval.tick() => {}
            }
            let snapshot = sim.read().await.snapshot();
            let runtime_state = *runtime.read().await;
            let values = vec![
                (
                    &nodes.endpoint_url,
                    DataValue::new_now(endpoint_url.clone()),
                ),
                (
                    &nodes.security_mode,
                    DataValue::new_now(security_profile.clone()),
                ),
                (
                    &nodes.service_state,
                    DataValue::new_now(state_for_runtime(runtime_state)),
                ),
                (&nodes.deployed, DataValue::new_now(runtime_state.deployed)),
                (&nodes.running, DataValue::new_now(runtime_state.running)),
                (
                    &nodes.ambient_temp_c,
                    DataValue::new_now(snapshot.mars_ambient_temp_c),
                ),
                (
                    &nodes.habitat_temp_c,
                    DataValue::new_now(snapshot.habitat_temp_c),
                ),
                (
                    &nodes.coolant_supply_temp_c,
                    DataValue::new_now(snapshot.coolant_supply_temp_c),
                ),
                (
                    &nodes.coolant_return_temp_c,
                    DataValue::new_now(snapshot.coolant_return_temp_c),
                ),
                (
                    &nodes.coolant_flow_kg_s,
                    DataValue::new_now(snapshot.coolant_flow_kg_s),
                ),
                (
                    &nodes.radiator_deployment_pct,
                    DataValue::new_now(snapshot.radiator_deployment_pct),
                ),
                (
                    &nodes.heat_load_kw,
                    DataValue::new_now(snapshot.equipment_heat_load_kw),
                ),
                (
                    &nodes.heat_rejection_kw,
                    DataValue::new_now(snapshot.heat_rejection_kw),
                ),
                (
                    &nodes.pump_power_kw,
                    DataValue::new_now(snapshot.pump_electric_power_kw),
                ),
                (
                    &nodes.balance_error_kw,
                    DataValue::new_now(snapshot.instantaneous_balance_error_kw),
                ),
                (
                    &nodes.cooling_available,
                    DataValue::new_now(snapshot.cooling_available),
                ),
                (
                    &nodes.alarm_habitat_hot,
                    DataValue::new_now(snapshot.alarm_habitat_hot),
                ),
                (
                    &nodes.alarm_habitat_cold,
                    DataValue::new_now(snapshot.alarm_habitat_cold),
                ),
                (
                    &nodes.alarm_coolant_hot,
                    DataValue::new_now(snapshot.alarm_coolant_hot),
                ),
            ];
            if let Err(err) = manager.set_values(
                &subscriptions,
                values.into_iter().map(|(node, value)| (node, None, value)),
            ) {
                warn!("Failed updating Thermal OPC UA values: {err}");
            }
        }
    });
    run_server(server, &config).await?;
    handle.cancel();
    let _ = sync_task.await;
    Ok(())
}

fn build_server(
    config: &SubsystemOpcuaConfig,
    namespace_uri: &str,
) -> anyhow::Result<(opcua::server::Server, opcua::server::ServerHandle)> {
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
        .application_name(config.application_name.clone())
        .application_uri(config.application_uri.clone())
        .product_uri("urn:underhill")
        .create_sample_keypair(true)
        .host(config.host.clone())
        .port(config.port)
        .pki_dir(config.pki_dir.clone())
        .trust_client_certs(true);

    let builder = match config.security_profile.as_str() {
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
        other => {
            return Err(anyhow::anyhow!(
                "Unsupported OPC UA security profile for subsystem server: {other}"
            ));
        }
    };

    builder
        .discovery_urls(vec![config.endpoint_path.clone()])
        .diagnostics_enabled(true)
        .with_node_manager(simple_node_manager(
            NamespaceMetadata {
                namespace_uri: namespace_uri.to_string(),
                ..Default::default()
            },
            "subsystem",
        ))
        .build()
        .map_err(anyhow::Error::msg)
}

async fn run_server(
    server: opcua::server::Server,
    config: &SubsystemOpcuaConfig,
) -> anyhow::Result<()> {
    let bind_addr = format!("{}:{}", config.bind_host, config.port);
    let use_default_listener = config.bind_host == config.host;
    info!(
        "OPC UA subsystem server binding on {}:{} and advertising {}",
        config.bind_host,
        config.port,
        config.endpoint_url()
    );
    if use_default_listener {
        server.run().await.map_err(anyhow::Error::msg)
    } else {
        let listener = TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bind OPC UA socket at {bind_addr}: {e}"))?;
        server.run_with(listener).await.map_err(anyhow::Error::msg)
    }
}

fn state_for_runtime(runtime_state: PeaRuntimeState) -> &'static str {
    if !runtime_state.deployed {
        "Stopped"
    } else if runtime_state.running {
        "Execute"
    } else {
        "Idle"
    }
}

fn build_eclss_address_space(
    ns: u16,
    manager: &Arc<opcua::server::node_manager::memory::InMemoryNodeManager<SimpleNodeManagerImpl>>,
) -> EclssNodes {
    let underhill = NodeId::new(ns, "Underhill");
    let pea = NodeId::new(ns, "Underhill.ECLSSPEA");
    let info = NodeId::new(ns, "Underhill.ECLSSPEA.InformationLabel");
    let diagnostics = NodeId::new(ns, "Underhill.ECLSSPEA.Diagnostics");
    let services = NodeId::new(ns, "Underhill.ECLSSPEA.Services");
    let service = NodeId::new(ns, "Underhill.ECLSSPEA.Services.EclssService");
    let service_control = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.Services.EclssService.ServiceControl",
    );
    let data = NodeId::new(ns, "Underhill.ECLSSPEA.DataAssemblies");
    let indicators = NodeId::new(ns, "Underhill.ECLSSPEA.DataAssemblies.Indicators");

    let mut address_space = manager.address_space().write();
    address_space.add_folder(
        &underhill,
        "Underhill",
        "Underhill",
        &NodeId::objects_folder_id(),
    );
    address_space.add_folder(&pea, "ECLSSPEA", "ECLSSPEA", &underhill);
    for (node, browse, parent) in [
        (&info, "InformationLabel", &pea),
        (&diagnostics, "Diagnostics", &pea),
        (&services, "Services", &pea),
        (&service, "EclssService", &services),
        (&service_control, "ServiceControl", &service),
        (&data, "DataAssemblies", &pea),
        (&indicators, "Indicators", &data),
    ] {
        address_space.add_folder(node, browse, browse, parent);
    }

    let endpoint_url = NodeId::new(ns, "Underhill.ECLSSPEA.Diagnostics.EndpointUrl");
    let security_mode = NodeId::new(ns, "Underhill.ECLSSPEA.Diagnostics.SecurityMode");
    let service_state = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.Services.EclssService.ServiceControl.State",
    );
    let deployed = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.Services.EclssService.ServiceControl.Deployed",
    );
    let running = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.Services.EclssService.ServiceControl.Running",
    );
    let o2_percent = NodeId::new(ns, "Underhill.ECLSSPEA.DataAssemblies.Indicators.O2Percent");
    let co2_ppm = NodeId::new(ns, "Underhill.ECLSSPEA.DataAssemblies.Indicators.CO2ppm");
    let humidity_pct = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.DataAssemblies.Indicators.HumidityPct",
    );
    let water_recovery_pct = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.DataAssemblies.Indicators.WaterRecoveryPct",
    );
    let power_kw = NodeId::new(ns, "Underhill.ECLSSPEA.DataAssemblies.Indicators.PowerKw");
    let alarm_high_co2 = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.DataAssemblies.Indicators.AlarmHighCO2",
    );
    let alarm_low_o2 = NodeId::new(
        ns,
        "Underhill.ECLSSPEA.DataAssemblies.Indicators.AlarmLowO2",
    );

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
        "NONE",
        false,
    );
    insert_var(
        &mut address_space,
        &service_control,
        &service_state,
        "State",
        "Idle",
        false,
    );
    insert_var(
        &mut address_space,
        &service_control,
        &deployed,
        "Deployed",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &service_control,
        &running,
        "Running",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &o2_percent,
        "O2Percent",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &co2_ppm,
        "CO2ppm",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &humidity_pct,
        "HumidityPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &water_recovery_pct,
        "WaterRecoveryPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &power_kw,
        "PowerKw",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &alarm_high_co2,
        "AlarmHighCO2",
        false,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &alarm_low_o2,
        "AlarmLowO2",
        false,
        false,
    );

    EclssNodes {
        endpoint_url,
        security_mode,
        service_state,
        deployed,
        running,
        o2_percent,
        co2_ppm,
        humidity_pct,
        water_recovery_pct,
        power_kw,
        alarm_high_co2,
        alarm_low_o2,
    }
}

fn build_sabatier_address_space(
    ns: u16,
    manager: &Arc<opcua::server::node_manager::memory::InMemoryNodeManager<SimpleNodeManagerImpl>>,
) -> SabatierNodes {
    let underhill = NodeId::new(ns, "Underhill");
    let pea = NodeId::new(ns, "Underhill.SabatierPEA");
    let info = NodeId::new(ns, "Underhill.SabatierPEA.InformationLabel");
    let diagnostics = NodeId::new(ns, "Underhill.SabatierPEA.Diagnostics");
    let services = NodeId::new(ns, "Underhill.SabatierPEA.Services");
    let service = NodeId::new(ns, "Underhill.SabatierPEA.Services.SabatierService");
    let service_control = NodeId::new(
        ns,
        "Underhill.SabatierPEA.Services.SabatierService.ServiceControl",
    );
    let data = NodeId::new(ns, "Underhill.SabatierPEA.DataAssemblies");
    let indicators = NodeId::new(ns, "Underhill.SabatierPEA.DataAssemblies.Indicators");

    let mut address_space = manager.address_space().write();
    address_space.add_folder(
        &underhill,
        "Underhill",
        "Underhill",
        &NodeId::objects_folder_id(),
    );
    address_space.add_folder(&pea, "SabatierPEA", "SabatierPEA", &underhill);
    for (node, browse, parent) in [
        (&info, "InformationLabel", &pea),
        (&diagnostics, "Diagnostics", &pea),
        (&services, "Services", &pea),
        (&service, "SabatierService", &services),
        (&service_control, "ServiceControl", &service),
        (&data, "DataAssemblies", &pea),
        (&indicators, "Indicators", &data),
    ] {
        address_space.add_folder(node, browse, browse, parent);
    }

    let endpoint_url = NodeId::new(ns, "Underhill.SabatierPEA.Diagnostics.EndpointUrl");
    let security_mode = NodeId::new(ns, "Underhill.SabatierPEA.Diagnostics.SecurityMode");
    let service_state = NodeId::new(
        ns,
        "Underhill.SabatierPEA.Services.SabatierService.ServiceControl.State",
    );
    let deployed = NodeId::new(
        ns,
        "Underhill.SabatierPEA.Services.SabatierService.ServiceControl.Deployed",
    );
    let running = NodeId::new(
        ns,
        "Underhill.SabatierPEA.Services.SabatierService.ServiceControl.Running",
    );
    let reactor_temp_c = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.ReactorTempC",
    );
    let reactor_pressure_bar = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.ReactorPressureBar",
    );
    let methane_production_kgph = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.MethaneProductionKgph",
    );
    let water_production_kgph = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.WaterProductionKgph",
    );
    let conversion_efficiency_pct = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.ConversionEfficiencyPct",
    );
    let catalyst_health_pct = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.CatalystHealthPct",
    );
    let power_kw = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.PowerKw",
    );
    let alarm_reactor_temp = NodeId::new(
        ns,
        "Underhill.SabatierPEA.DataAssemblies.Indicators.AlarmReactorTemp",
    );

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
        "NONE",
        false,
    );
    insert_var(
        &mut address_space,
        &service_control,
        &service_state,
        "State",
        "Idle",
        false,
    );
    insert_var(
        &mut address_space,
        &service_control,
        &deployed,
        "Deployed",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &service_control,
        &running,
        "Running",
        true,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &reactor_temp_c,
        "ReactorTempC",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &reactor_pressure_bar,
        "ReactorPressureBar",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &methane_production_kgph,
        "MethaneProductionKgph",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &water_production_kgph,
        "WaterProductionKgph",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &conversion_efficiency_pct,
        "ConversionEfficiencyPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &catalyst_health_pct,
        "CatalystHealthPct",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &power_kw,
        "PowerKw",
        0.0f64,
        false,
    );
    insert_var(
        &mut address_space,
        &indicators,
        &alarm_reactor_temp,
        "AlarmReactorTemp",
        false,
        false,
    );

    SabatierNodes {
        endpoint_url,
        security_mode,
        service_state,
        deployed,
        running,
        reactor_temp_c,
        reactor_pressure_bar,
        methane_production_kgph,
        water_production_kgph,
        conversion_efficiency_pct,
        catalyst_health_pct,
        power_kw,
        alarm_reactor_temp,
    }
}

fn build_power_address_space(
    ns: u16,
    manager: &Arc<opcua::server::node_manager::memory::InMemoryNodeManager<SimpleNodeManagerImpl>>,
) -> PowerNodes {
    let underhill = NodeId::new(ns, "Underhill");
    let pea = NodeId::new(ns, "Underhill.PowerPEA");
    let diagnostics = NodeId::new(ns, "Underhill.PowerPEA.Diagnostics");
    let services = NodeId::new(ns, "Underhill.PowerPEA.Services");
    let service = NodeId::new(ns, "Underhill.PowerPEA.Services.PowerService");
    let control = NodeId::new(
        ns,
        "Underhill.PowerPEA.Services.PowerService.ServiceControl",
    );
    let data = NodeId::new(ns, "Underhill.PowerPEA.DataAssemblies");
    let indicators = NodeId::new(ns, "Underhill.PowerPEA.DataAssemblies.Indicators");
    let alarms = NodeId::new(ns, "Underhill.PowerPEA.DataAssemblies.Alarms");
    let mut space = manager.address_space().write();
    space.add_folder(
        &underhill,
        "Underhill",
        "Underhill",
        &NodeId::objects_folder_id(),
    );
    space.add_folder(&pea, "PowerPEA", "PowerPEA", &underhill);
    for (node, browse, parent) in [
        (&diagnostics, "Diagnostics", &pea),
        (&services, "Services", &pea),
        (&service, "PowerService", &services),
        (&control, "ServiceControl", &service),
        (&data, "DataAssemblies", &pea),
        (&indicators, "Indicators", &data),
        (&alarms, "Alarms", &data),
    ] {
        space.add_folder(node, browse, browse, parent);
    }
    let endpoint_url = NodeId::new(ns, "Underhill.PowerPEA.Diagnostics.EndpointUrl");
    let security_mode = NodeId::new(ns, "Underhill.PowerPEA.Diagnostics.SecurityMode");
    let service_state = NodeId::new(
        ns,
        "Underhill.PowerPEA.Services.PowerService.ServiceControl.State",
    );
    let deployed = NodeId::new(
        ns,
        "Underhill.PowerPEA.Services.PowerService.ServiceControl.Deployed",
    );
    let running = NodeId::new(
        ns,
        "Underhill.PowerPEA.Services.PowerService.ServiceControl.Running",
    );
    let generation_kw = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.GenerationKw",
    );
    let requested_load_kw = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.RequestedLoadKw",
    );
    let served_load_kw = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.ServedLoadKw",
    );
    let battery_power_kw = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.BatteryPowerKw",
    );
    let battery_energy_kwh = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.BatteryEnergyKwh",
    );
    let battery_soc_pct = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.BatterySocPct",
    );
    let bus_voltage_v = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.BusVoltageV",
    );
    let unmet_load_kw = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Indicators.UnmetLoadKw",
    );
    let load_shed_active = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Alarms.LoadShedActive",
    );
    let alarm_battery_low = NodeId::new(ns, "Underhill.PowerPEA.DataAssemblies.Alarms.BatteryLow");
    let alarm_bus_undervoltage = NodeId::new(
        ns,
        "Underhill.PowerPEA.DataAssemblies.Alarms.BusUndervoltage",
    );
    let balance_error_kw = NodeId::new(
        ns,
        "Underhill.PowerPEA.Diagnostics.InstantaneousBalanceErrorKw",
    );
    for (parent, node, browse, value) in [
        (
            &diagnostics,
            &endpoint_url,
            "EndpointUrl",
            Variant::from(""),
        ),
        (
            &diagnostics,
            &security_mode,
            "SecurityMode",
            Variant::from("NONE"),
        ),
        (&control, &service_state, "State", Variant::from("Idle")),
    ] {
        insert_var(&mut space, parent, node, browse, value, false);
    }
    insert_var(&mut space, &control, &deployed, "Deployed", true, false);
    insert_var(&mut space, &control, &running, "Running", true, false);
    for (node, browse) in [
        (&generation_kw, "GenerationKw"),
        (&requested_load_kw, "RequestedLoadKw"),
        (&served_load_kw, "ServedLoadKw"),
        (&battery_power_kw, "BatteryPowerKw"),
        (&battery_energy_kwh, "BatteryEnergyKwh"),
        (&battery_soc_pct, "BatterySocPct"),
        (&bus_voltage_v, "BusVoltageV"),
        (&unmet_load_kw, "UnmetLoadKw"),
    ] {
        insert_var(&mut space, &indicators, node, browse, 0.0f64, false);
    }
    insert_var(
        &mut space,
        &alarms,
        &load_shed_active,
        "LoadShedActive",
        false,
        false,
    );
    insert_var(
        &mut space,
        &alarms,
        &alarm_battery_low,
        "BatteryLow",
        false,
        false,
    );
    insert_var(
        &mut space,
        &alarms,
        &alarm_bus_undervoltage,
        "BusUndervoltage",
        false,
        false,
    );
    insert_var(
        &mut space,
        &diagnostics,
        &balance_error_kw,
        "InstantaneousBalanceErrorKw",
        0.0f64,
        false,
    );
    PowerNodes {
        endpoint_url,
        security_mode,
        service_state,
        deployed,
        running,
        generation_kw,
        requested_load_kw,
        served_load_kw,
        battery_power_kw,
        battery_energy_kwh,
        battery_soc_pct,
        bus_voltage_v,
        unmet_load_kw,
        load_shed_active,
        alarm_battery_low,
        alarm_bus_undervoltage,
        balance_error_kw,
    }
}

fn build_thermal_address_space(
    ns: u16,
    manager: &Arc<opcua::server::node_manager::memory::InMemoryNodeManager<SimpleNodeManagerImpl>>,
) -> ThermalNodes {
    let underhill = NodeId::new(ns, "Underhill");
    let pea = NodeId::new(ns, "Underhill.ThermalPEA");
    let diagnostics = NodeId::new(ns, "Underhill.ThermalPEA.Diagnostics");
    let services = NodeId::new(ns, "Underhill.ThermalPEA.Services");
    let service = NodeId::new(ns, "Underhill.ThermalPEA.Services.ThermalService");
    let control = NodeId::new(
        ns,
        "Underhill.ThermalPEA.Services.ThermalService.ServiceControl",
    );
    let data = NodeId::new(ns, "Underhill.ThermalPEA.DataAssemblies");
    let indicators = NodeId::new(ns, "Underhill.ThermalPEA.DataAssemblies.Indicators");
    let alarms = NodeId::new(ns, "Underhill.ThermalPEA.DataAssemblies.Alarms");
    let mut space = manager.address_space().write();
    space.add_folder(
        &underhill,
        "Underhill",
        "Underhill",
        &NodeId::objects_folder_id(),
    );
    space.add_folder(&pea, "ThermalPEA", "ThermalPEA", &underhill);
    for (node, browse, parent) in [
        (&diagnostics, "Diagnostics", &pea),
        (&services, "Services", &pea),
        (&service, "ThermalService", &services),
        (&control, "ServiceControl", &service),
        (&data, "DataAssemblies", &pea),
        (&indicators, "Indicators", &data),
        (&alarms, "Alarms", &data),
    ] {
        space.add_folder(node, browse, browse, parent);
    }
    let node = |suffix: &str| NodeId::new(ns, format!("Underhill.ThermalPEA.{suffix}"));
    let endpoint_url = node("Diagnostics.EndpointUrl");
    let security_mode = node("Diagnostics.SecurityMode");
    let service_state = node("Services.ThermalService.ServiceControl.State");
    let deployed = node("Services.ThermalService.ServiceControl.Deployed");
    let running = node("Services.ThermalService.ServiceControl.Running");
    let ambient_temp_c = node("DataAssemblies.Indicators.MarsAmbientTempC");
    let habitat_temp_c = node("DataAssemblies.Indicators.HabitatTempC");
    let coolant_supply_temp_c = node("DataAssemblies.Indicators.CoolantSupplyTempC");
    let coolant_return_temp_c = node("DataAssemblies.Indicators.CoolantReturnTempC");
    let coolant_flow_kg_s = node("DataAssemblies.Indicators.CoolantFlowKgS");
    let radiator_deployment_pct = node("DataAssemblies.Indicators.RadiatorDeploymentPct");
    let heat_load_kw = node("DataAssemblies.Indicators.HeatLoadKw");
    let heat_rejection_kw = node("DataAssemblies.Indicators.HeatRejectionKw");
    let pump_power_kw = node("DataAssemblies.Indicators.PumpPowerKw");
    let balance_error_kw = node("Diagnostics.InstantaneousBalanceErrorKw");
    let cooling_available = node("DataAssemblies.Alarms.CoolingAvailable");
    let alarm_habitat_hot = node("DataAssemblies.Alarms.HabitatHot");
    let alarm_habitat_cold = node("DataAssemblies.Alarms.HabitatCold");
    let alarm_coolant_hot = node("DataAssemblies.Alarms.CoolantHot");
    insert_var(
        &mut space,
        &diagnostics,
        &endpoint_url,
        "EndpointUrl",
        "",
        false,
    );
    insert_var(
        &mut space,
        &diagnostics,
        &security_mode,
        "SecurityMode",
        "NONE",
        false,
    );
    insert_var(&mut space, &control, &service_state, "State", "Idle", false);
    insert_var(&mut space, &control, &deployed, "Deployed", true, false);
    insert_var(&mut space, &control, &running, "Running", true, false);
    for (id, browse) in [
        (&ambient_temp_c, "MarsAmbientTempC"),
        (&habitat_temp_c, "HabitatTempC"),
        (&coolant_supply_temp_c, "CoolantSupplyTempC"),
        (&coolant_return_temp_c, "CoolantReturnTempC"),
        (&coolant_flow_kg_s, "CoolantFlowKgS"),
        (&radiator_deployment_pct, "RadiatorDeploymentPct"),
        (&heat_load_kw, "HeatLoadKw"),
        (&heat_rejection_kw, "HeatRejectionKw"),
        (&pump_power_kw, "PumpPowerKw"),
    ] {
        insert_var(&mut space, &indicators, id, browse, 0.0f64, false);
    }
    insert_var(
        &mut space,
        &diagnostics,
        &balance_error_kw,
        "InstantaneousBalanceErrorKw",
        0.0f64,
        false,
    );
    insert_var(
        &mut space,
        &alarms,
        &cooling_available,
        "CoolingAvailable",
        false,
        false,
    );
    insert_var(
        &mut space,
        &alarms,
        &alarm_habitat_hot,
        "HabitatHot",
        false,
        false,
    );
    insert_var(
        &mut space,
        &alarms,
        &alarm_habitat_cold,
        "HabitatCold",
        false,
        false,
    );
    insert_var(
        &mut space,
        &alarms,
        &alarm_coolant_hot,
        "CoolantHot",
        false,
        false,
    );
    ThermalNodes {
        endpoint_url,
        security_mode,
        service_state,
        deployed,
        running,
        ambient_temp_c,
        habitat_temp_c,
        coolant_supply_temp_c,
        coolant_return_temp_c,
        coolant_flow_kg_s,
        radiator_deployment_pct,
        heat_load_kw,
        heat_rejection_kw,
        pump_power_kw,
        balance_error_kw,
        cooling_available,
        alarm_habitat_hot,
        alarm_habitat_cold,
        alarm_coolant_hot,
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
