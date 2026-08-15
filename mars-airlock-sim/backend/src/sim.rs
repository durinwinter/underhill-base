use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::model::{
    ActiveCommand, AlarmState, ClientSession, CommandChannelState, CommandEnum,
    CommandRequestFields, CommandResponseFields, CommandSourceEnum, CommandStatusEnum,
    DiagnosticsState, EventEntry, LeakRateUpdateRequest, MtpModes, MtpModesUpdateRequest,
    MtpProcedureRuntime, MtpRuntime, MtpStateMachine, MtpTreeResponse, PeaInformationLabel,
    Permissions, PermissionsUpdateRequest, ProcedureRequest, ProcedureRuntime, ProcedureState,
    ServiceInformation, ServiceState, Snapshot, ValveFaultUpdateRequest,
};

use serde_json::json;

const PROC_DEPRESSURIZE: &str = "Proc_DepressurizeForEVA";
const PROC_PRESSURIZE: &str = "Proc_PressurizeForEntry";
const PROC_MANUAL_JOG: &str = "Proc_ManualDoorJog";
const FIXED_TIMESTEP_SEC: f64 = 0.05;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ValveActuatorDynamics {
    initialized: bool,
    command_pct: f64,
    sensed_pct: f64,
    max_travel_rate_pct_per_sec: f64,
    response_time_constant_sec: f64,
    deadband_pct: f64,
    stiction_breakaway_pct: f64,
    sensor_bias_pct: f64,
    hard_stuck: bool,
    leakage_pct: f64,
    in_motion: bool,
    stiction_active: bool,
}

impl Default for ValveActuatorDynamics {
    fn default() -> Self {
        Self {
            initialized: false,
            command_pct: 0.0,
            sensed_pct: 0.0,
            max_travel_rate_pct_per_sec: 35.0,
            response_time_constant_sec: 0.4,
            deadband_pct: 0.25,
            stiction_breakaway_pct: 0.0,
            sensor_bias_pct: 0.0,
            hard_stuck: false,
            leakage_pct: 0.0,
            in_motion: false,
            stiction_active: false,
        }
    }
}

impl ValveActuatorDynamics {
    fn new_at(position_pct: f64) -> Self {
        Self {
            initialized: true,
            command_pct: position_pct,
            sensed_pct: position_pct,
            ..Self::default()
        }
    }

    fn prepare_after_restore(&mut self, actual_position_pct: f64) {
        if !self.initialized {
            self.initialized = true;
            self.command_pct = actual_position_pct;
            self.sensed_pct = actual_position_pct;
        }
    }

    fn step(&mut self, actual_position_pct: &mut f64, dt_sec: f64) {
        let error = self.command_pct - *actual_position_pct;
        if self.hard_stuck {
            self.in_motion = false;
            self.stiction_active = error.abs() > self.deadband_pct;
        } else {
            if !self.in_motion && error.abs() > self.stiction_breakaway_pct {
                self.in_motion = true;
            }
            if self.in_motion {
                let desired_rate = error / self.response_time_constant_sec.max(0.01);
                let rate = desired_rate.clamp(
                    -self.max_travel_rate_pct_per_sec,
                    self.max_travel_rate_pct_per_sec,
                );
                *actual_position_pct = (*actual_position_pct + rate * dt_sec).clamp(0.0, 100.0);
                if (self.command_pct - *actual_position_pct).abs() <= self.deadband_pct {
                    *actual_position_pct = self.command_pct;
                    self.in_motion = false;
                }
            }
            self.stiction_active = !self.in_motion
                && (self.command_pct - *actual_position_pct).abs() > self.deadband_pct;
        }
        self.sensed_pct = (*actual_position_pct + self.sensor_bias_pct).clamp(0.0, 100.0);
    }

    fn effective_flow_position_pct(&self, actual_position_pct: f64) -> f64 {
        actual_position_pct.max(self.leakage_pct).clamp(0.0, 100.0)
    }

    fn residual_pct(&self) -> f64 {
        self.command_pct - self.sensed_pct
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Simulation {
    sim_time_sec: f64,
    state_name: String,
    pressure_pa: f64,
    temperature_k: f64,
    o2_percent: f64,
    inner_door_position_pct: f64,
    outer_door_position_pct: f64,
    inner_door_target_pct: f64,
    outer_door_target_pct: f64,
    inner_lock_engaged: bool,
    outer_lock_engaged: bool,
    equalize_valve_pct: f64,
    vent_valve_pct: f64,
    #[serde(default)]
    equalize_valve_dynamics: ValveActuatorDynamics,
    #[serde(default)]
    vent_valve_dynamics: ValveActuatorDynamics,
    pump_on: bool,
    pump_current_a: f64,
    leak_rate_nominal: f64,
    outer_unlock_max_pressure_pa: f64,
    inner_unlock_min_pressure_pa: f64,
    high_pressure_alarm_pa: f64,
    low_pressure_alarm_pa: f64,
    active_start_metric: f64,
    active_target_metric: f64,
    alarms: AlarmState,
    permissions: Permissions,
    diagnostics: DiagnosticsState,
    mtp_modes: MtpModes,
    mtp_state_machine: MtpStateMachine,
    mtp_runtime: MtpRuntime,
    event_log: Vec<EventEntry>,
    operator_channel: CommandChannelState,
    remote_channel: CommandChannelState,
    active_command: ActiveCommand,
    last_operator_execute: bool,
    last_remote_execute: bool,
    // procedure integration
    pending_procedure_requests: Vec<ProcedureRequest>,
    pub(crate) procedure_runtimes: std::collections::HashMap<String, MtpProcedureRuntime>,
}

impl Simulation {
    pub fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }

    pub fn normalize_security_profile(profile: &str) -> Option<String> {
        let normalized = profile.trim().to_uppercase();
        if matches!(normalized.as_str(), "NONE" | "BASIC256SHA256" | "BOTH") {
            Some(normalized)
        } else {
            None
        }
    }

    pub fn prepare_after_restore(&mut self, endpoint_url: String) {
        let now = Self::now_ms();
        self.diagnostics.endpoint_url = endpoint_url;
        self.diagnostics.server_start_time_ms = now;
        self.diagnostics.server_uptime_sec = 0.0;
        self.diagnostics.connected_client_count = 0;
        self.diagnostics.connected_client_summary = "none".to_string();
        self.diagnostics.connected_clients.clear();
        self.diagnostics.subscription_count = 0;
        self.equalize_valve_dynamics
            .prepare_after_restore(self.equalize_valve_pct);
        self.vent_valve_dynamics
            .prepare_after_restore(self.vent_valve_pct);
        self.log_event("INFO", "SYSTEM", "Plant state restored from checkpoint");
    }

    fn security_modes_for(profile: &str) -> Vec<String> {
        match profile {
            "NONE" => vec!["NONE".to_string()],
            "BASIC256SHA256" => vec!["BASIC256SHA256".to_string()],
            "BOTH" => vec!["NONE".to_string(), "BASIC256SHA256".to_string()],
            _ => vec!["NONE".to_string()],
        }
    }

    pub fn new(active_security_mode: String, endpoint_url: String) -> Self {
        let now = Self::now_ms();
        let active_security_mode = Self::normalize_security_profile(&active_security_mode)
            .unwrap_or_else(|| "NONE".to_string());
        let security_modes_enabled = Self::security_modes_for(&active_security_mode);

        let diagnostics = DiagnosticsState {
            security_modes_enabled,
            active_security_mode,
            endpoint_url,
            server_start_time_ms: now,
            ..DiagnosticsState::default()
        };

        let mtp_runtime = MtpRuntime {
            pea_information_label: PeaInformationLabel {
                tag_name: "AIRLOCK-PEA-001".to_string(),
                module_name: "Underhill Airlock".to_string(),
                manufacturer: "MarsSimWorks".to_string(),
                serial_number: "UA-2026-0001".to_string(),
                hardware_revision: "rev-a".to_string(),
                software_revision: "0.1.0".to_string(),
                mtp_version: "VDI/VDE/NAMUR 2658 (conceptual)".to_string(),
                opcuaruntime_version: "simulated-runtime".to_string(),
                build_timestamp_ms: now,
                documentation_uri: "https://example.invalid/mars-airlock".to_string(),
                health_state: "OK".to_string(),
            },
            service_information: ServiceInformation {
                service_name: "AirlockService".to_string(),
                service_id: 1,
                service_revision: "1.0".to_string(),
                active_procedure: "None".to_string(),
            },
            procedures: vec![
                ProcedureRuntime {
                    id: 1,
                    name: PROC_DEPRESSURIZE.to_string(),
                    proc_state: "idle".to_string(),
                    progress_pct: 0.0,
                    last_result: String::new(),
                },
                ProcedureRuntime {
                    id: 2,
                    name: PROC_PRESSURIZE.to_string(),
                    proc_state: "idle".to_string(),
                    progress_pct: 0.0,
                    last_result: String::new(),
                },
                ProcedureRuntime {
                    id: 3,
                    name: PROC_MANUAL_JOG.to_string(),
                    proc_state: "idle".to_string(),
                    progress_pct: 0.0,
                    last_result: String::new(),
                },
            ],
        };

        let mut simulation = Self {
            sim_time_sec: 0.0,
            state_name: "idle".to_string(),
            pressure_pa: 101_325.0,
            temperature_k: 293.15,
            o2_percent: 21.0,
            inner_door_position_pct: 0.0,
            outer_door_position_pct: 0.0,
            inner_door_target_pct: 0.0,
            outer_door_target_pct: 0.0,
            inner_lock_engaged: true,
            outer_lock_engaged: true,
            equalize_valve_pct: 0.0,
            vent_valve_pct: 0.0,
            equalize_valve_dynamics: ValveActuatorDynamics::new_at(0.0),
            vent_valve_dynamics: ValveActuatorDynamics::new_at(0.0),
            pump_on: false,
            pump_current_a: 0.0,
            leak_rate_nominal: 0.0005,
            outer_unlock_max_pressure_pa: 5_000.0,
            inner_unlock_min_pressure_pa: 90_000.0,
            high_pressure_alarm_pa: 110_000.0,
            low_pressure_alarm_pa: 2_000.0,
            active_start_metric: 0.0,
            active_target_metric: 0.0,
            alarms: AlarmState::default(),
            permissions: Permissions::default(),
            diagnostics,
            mtp_modes: MtpModes::default(),
            mtp_state_machine: MtpStateMachine::default(),
            mtp_runtime,
            event_log: Vec::new(),
            operator_channel: CommandChannelState::default(),
            remote_channel: CommandChannelState::default(),
            active_command: ActiveCommand::default(),
            last_operator_execute: false,
            last_remote_execute: false,
            // initialize MTP procedure handling
            pending_procedure_requests: Vec::new(),
            procedure_runtimes: std::collections::HashMap::new(),
        };

        simulation.log_event("INFO", "SYSTEM", "Simulation initialized");
        simulation
    }

    pub fn set_security_profile(&mut self, profile: &str) {
        let Some(normalized) = Self::normalize_security_profile(profile) else {
            self.diagnostics.last_error = format!("Unsupported security profile: {profile}");
            self.log_event(
                "WARN",
                "SYSTEM",
                format!("Unsupported security profile requested: {profile}"),
            );
            return;
        };

        let changed = normalized != self.diagnostics.active_security_mode;
        self.diagnostics.active_security_mode = normalized.clone();
        self.diagnostics.security_modes_enabled = Self::security_modes_for(&normalized);

        self.diagnostics.last_error.clear();
        if changed {
            self.log_event(
                "INFO",
                "SYSTEM",
                format!("Security profile switched to {normalized}"),
            );
        }
    }

    pub fn set_permissions(&mut self, request: PermissionsUpdateRequest) {
        if let Some(value) = request.operator_control_enabled {
            self.permissions.operator_control_enabled = value;
            self.log_event(
                "INFO",
                "SYSTEM",
                format!("Operator control enabled set to {value}"),
            );
        }

        if let Some(value) = request.remote_control_enabled {
            self.permissions.remote_control_enabled = value;
            self.log_event(
                "INFO",
                "SYSTEM",
                format!("Remote control enabled set to {value}"),
            );
        }
    }

    pub fn set_modes(&mut self, request: MtpModesUpdateRequest) {
        if let Some(mode) = request.operation_mode {
            self.mtp_modes.operation_mode = mode;
            self.log_event(
                "INFO",
                "SYSTEM",
                format!("Operation mode changed to {:?}", mode),
            );
        }

        if let Some(command_en) = request.command_en {
            self.mtp_modes.command_en = command_en;
            self.log_event("INFO", "SYSTEM", format!("CommandEn set to {command_en}"));
        }

        if let Some(reason) = request.command_en_reason {
            self.mtp_modes.command_en_reason = reason;
        }
    }

    pub fn set_leak_rate(&mut self, request: LeakRateUpdateRequest) {
        self.leak_rate_nominal = request.leak_rate_nominal.clamp(0.0, 0.01);
        self.log_event(
            "WARN",
            "SYSTEM",
            format!("Leak rate override set to {:.6}", self.leak_rate_nominal),
        );
    }

    pub fn events(&self) -> Vec<EventEntry> {
        self.event_log.clone()
    }

    // ------------------------------------------------------------------------
    // Procedure request handling (MTP calling convention)
    // ------------------------------------------------------------------------

    pub fn request_procedure(&mut self, req: ProcedureRequest) {
        // store request for processing
        self.pending_procedure_requests.push(req);
        self.log_event("INFO", "MTP", "Procedure request queued");
    }

    fn process_procedure_requests(&mut self) {
        for req in self.pending_procedure_requests.drain(..) {
            // create or update runtime entry
            let key = format!("{}/{}/{}", req.pea_id, req.service_name, req.procedure_name);
            let runtime = self
                .procedure_runtimes
                .entry(key.clone())
                .or_insert_with(|| MtpProcedureRuntime {
                    current_request_id: 0,
                    state: ProcedureState::Idle,
                    progress_pct: 0.0,
                    result: std::collections::HashMap::new(),
                    error_message: None,
                    started_at_ms: 0,
                    completed_at_ms: 0,
                });
            // start execution
            runtime.current_request_id = 0;
            runtime.state = ProcedureState::Running;
            runtime.progress_pct = 0.0;
            runtime.started_at_ms = Self::now_ms();
            runtime.result.clear();
            runtime.error_message = None;
        }
    }

    fn advance_procedures(&mut self, dt_sec: f64) {
        let now = Self::now_ms();
        for runtime in self.procedure_runtimes.values_mut() {
            if runtime.state == ProcedureState::Running {
                runtime.progress_pct += dt_sec * 20.0; // arbitrary 5s total
                if runtime.progress_pct >= 100.0 {
                    runtime.progress_pct = 100.0;
                    runtime.state = ProcedureState::Done;
                    runtime.completed_at_ms = now;
                    // stub result
                    runtime
                        .result
                        .insert("status".to_string(), json!("success"));
                    runtime
                        .result
                        .insert("duration_sec".to_string(), json!(5.0));
                }
            }
        }
    }

    pub fn register_client(
        &mut self,
        session_id: u64,
        session_name: String,
        remote_address: String,
    ) {
        let now = Self::now_ms();
        self.diagnostics.connected_clients.push(ClientSession {
            session_id,
            application_name: "mars-airlock-web".to_string(),
            session_name: session_name.clone(),
            remote_address,
            last_activity_time_ms: now,
        });
        self.refresh_client_counts();
        self.log_event(
            "INFO",
            "SYSTEM",
            format!("Client connected: {session_name}"),
        );
    }

    pub fn touch_client(&mut self, session_id: u64) {
        let now = Self::now_ms();
        if let Some(client) = self
            .diagnostics
            .connected_clients
            .iter_mut()
            .find(|entry| entry.session_id == session_id)
        {
            client.last_activity_time_ms = now;
        }
    }

    pub fn unregister_client(&mut self, session_id: u64) {
        self.diagnostics
            .connected_clients
            .retain(|entry| entry.session_id != session_id);
        self.refresh_client_counts();
        self.log_event(
            "INFO",
            "SYSTEM",
            format!("Client disconnected: {session_id}"),
        );
    }

    pub fn set_opcua_diagnostics(
        &mut self,
        server_start_time_ms: u64,
        server_uptime_sec: f64,
        current_session_count: u32,
        current_subscription_count: u32,
    ) {
        self.diagnostics.server_start_time_ms = server_start_time_ms;
        self.diagnostics.server_uptime_sec = server_uptime_sec.max(0.0);
        self.diagnostics.connected_client_count = current_session_count;
        self.diagnostics.subscription_count = current_subscription_count;
        let web_clients = self.diagnostics.connected_clients.len() as u32;
        self.diagnostics.connected_client_summary = format!(
            "opcua_sessions={current_session_count}, opcua_subscriptions={current_subscription_count}, web_sessions={web_clients}"
        );
    }

    pub fn write_request(
        &mut self,
        source: CommandSourceEnum,
        request: CommandRequestFields,
    ) -> CommandResponseFields {
        let previous_execute = self.last_execute_for(source);
        self.set_last_execute_for(source, request.execute);

        {
            let channel = self.channel_mut(source);
            channel.req = request.clone();
        }

        let rising_edge = !previous_execute && request.execute;
        if !rising_edge {
            return self.channel(source).rsp.clone();
        }

        let last_sequence = self.channel(source).last_sequence_processed;
        if request.sequence_id == last_sequence {
            let response = CommandResponseFields {
                ack_sequence_id: request.sequence_id,
                status: CommandStatusEnum::Rejected,
                reject_reason: "Duplicate sequence ignored".to_string(),
                last_update_time_ms: Self::now_ms(),
            };
            self.set_channel_response(source, response.clone());
            self.log_event(
                "WARN",
                source.as_str(),
                format!(
                    "Rejected duplicate sequence {} for {}",
                    request.sequence_id,
                    display_command(request.command)
                ),
            );
            return response;
        }

        if let Some(reason) = self.validate_request(source, &request) {
            let response = CommandResponseFields {
                ack_sequence_id: request.sequence_id,
                status: CommandStatusEnum::Rejected,
                reject_reason: reason.clone(),
                last_update_time_ms: Self::now_ms(),
            };
            self.set_channel_response(source, response.clone());
            self.diagnostics.last_rejected_command = format!(
                "{}:{}:{}",
                source.as_str(),
                display_command(request.command),
                reason
            );
            self.alarms.interlock_violation = reason.starts_with("Interlock");
            self.mtp_state_machine.blocking_condition = reason.clone();
            self.log_event(
                "WARN",
                source.as_str(),
                format!("{} rejected: {reason}", display_command(request.command)),
            );
            return response;
        }

        {
            let channel = self.channel_mut(source);
            channel.last_sequence_processed = request.sequence_id;
            channel.rsp = CommandResponseFields {
                ack_sequence_id: request.sequence_id,
                status: CommandStatusEnum::Accepted,
                reject_reason: String::new(),
                last_update_time_ms: Self::now_ms(),
            };
        }

        self.active_command = ActiveCommand {
            command: request.command,
            source,
            sequence_id: request.sequence_id,
            param1: request.param1,
            param2: request.param2,
            state: CommandStatusEnum::Running,
            progress_pct: 0.0,
            blocking_condition: String::new(),
            start_time_ms: Self::now_ms(),
            last_update_time_ms: Self::now_ms(),
        };

        self.mtp_modes.source_mode = source;
        self.mtp_state_machine.current_state = ServiceState::Execute;
        self.mtp_state_machine.transition_active = true;
        self.mtp_state_machine.time_in_state_sec = 0.0;
        self.mtp_state_machine.blocking_condition.clear();

        self.active_start_metric = self.pressure_pa;
        self.active_target_metric = self.pressure_pa;

        self.set_active_procedure_for_command(request.command);
        self.log_event(
            "INFO",
            source.as_str(),
            format!(
                "Accepted command {} (seq={})",
                display_command(request.command),
                request.sequence_id
            ),
        );

        if self.apply_immediate_command(request.command, request.param1, request.param2) {
            return self.channel(source).rsp.clone();
        }

        match request.command {
            CommandEnum::StartDepressurizeCycle => {
                self.active_start_metric = self.pressure_pa;
                self.active_target_metric = self.outer_unlock_max_pressure_pa;
                self.state_name = "cycle_depressurize".to_string();
            }
            CommandEnum::StartPressurizeCycle => {
                self.active_start_metric = self.pressure_pa;
                self.active_target_metric = self.inner_unlock_min_pressure_pa;
                self.state_name = "cycle_pressurize".to_string();
            }
            CommandEnum::SetInnerDoorTargetPct => {
                self.inner_door_target_pct = request.param1.clamp(0.0, 100.0);
                self.active_start_metric = self.inner_door_position_pct;
                self.active_target_metric = self.inner_door_target_pct;
                self.state_name = "manual_override".to_string();
            }
            CommandEnum::SetOuterDoorTargetPct => {
                self.outer_door_target_pct = request.param1.clamp(0.0, 100.0);
                self.active_start_metric = self.outer_door_position_pct;
                self.active_target_metric = self.outer_door_target_pct;
                self.state_name = "manual_override".to_string();
            }
            _ => {}
        }

        self.channel(source).rsp.clone()
    }

    pub fn step(&mut self, dt_sec: f64) {
        self.sim_time_sec += dt_sec;
        self.mtp_state_machine.time_in_state_sec += dt_sec;

        // process any pending procedure requests
        self.process_procedure_requests();

        // advance existing procedures
        self.advance_procedures(dt_sec);

        self.run_active_command(dt_sec);
        self.apply_physics(dt_sec);
        self.update_alarms();

        self.active_command.last_update_time_ms = Self::now_ms();
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            timestamp_ms: Self::now_ms(),
            sim_time_sec: self.sim_time_sec,
            pressure_pa: self.pressure_pa,
            temperature_k: self.temperature_k,
            o2_percent: self.o2_percent,
            leak_rate_nominal: self.leak_rate_nominal,
            fixed_timestep_sec: FIXED_TIMESTEP_SEC,
            outer_unlock_max_pressure_pa: self.outer_unlock_max_pressure_pa,
            inner_unlock_min_pressure_pa: self.inner_unlock_min_pressure_pa,
            high_pressure_alarm_pa: self.high_pressure_alarm_pa,
            low_pressure_alarm_pa: self.low_pressure_alarm_pa,
            inner_door_position_pct: self.inner_door_position_pct,
            outer_door_position_pct: self.outer_door_position_pct,
            inner_lock_engaged: self.inner_lock_engaged,
            outer_lock_engaged: self.outer_lock_engaged,
            equalize_valve_pct: self.equalize_valve_pct,
            vent_valve_pct: self.vent_valve_pct,
            equalize_valve_command_pct: self.equalize_valve_dynamics.command_pct,
            equalize_valve_sensed_pct: self.equalize_valve_dynamics.sensed_pct,
            equalize_valve_residual_pct: self.equalize_valve_dynamics.residual_pct(),
            equalize_valve_stiction_active: self.equalize_valve_dynamics.stiction_active,
            vent_valve_command_pct: self.vent_valve_dynamics.command_pct,
            vent_valve_sensed_pct: self.vent_valve_dynamics.sensed_pct,
            vent_valve_residual_pct: self.vent_valve_dynamics.residual_pct(),
            vent_valve_stiction_active: self.vent_valve_dynamics.stiction_active,
            pump_on: self.pump_on,
            pump_current_a: self.pump_current_a,
            state_name: self.state_name.clone(),
            active_command: self.active_command.clone(),
            alarms: self.alarms.clone(),
            permissions: self.permissions.clone(),
            diagnostics: self.diagnostics.clone(),
            mtp_modes: self.mtp_modes.clone(),
            mtp_state_machine: self.mtp_state_machine.clone(),
            mtp_runtime: self.mtp_runtime.clone(),
            event_log: self.event_log.clone(),
            operator_channel: self.operator_channel.clone(),
            remote_channel: self.remote_channel.clone(),
            // new procedure fields are not included in snapshot
        }
    }

    pub fn mtp_tree(&self) -> MtpTreeResponse {
        MtpTreeResponse {
            namespace: "urn:mars-airlock:mtp".to_string(),
            root_path: "Objects/MarsBase/AirlockPEA".to_string(),
            nodes: vec![
                "PEAInformationLabel".to_string(),
                "Diagnostics".to_string(),
                "ServiceSet/AirlockService/ServiceInformation".to_string(),
                "ServiceSet/AirlockService/Modes".to_string(),
                "ServiceSet/AirlockService/StateMachine".to_string(),
                "ServiceSet/AirlockService/Procedures/Proc_DepressurizeForEVA".to_string(),
                "ServiceSet/AirlockService/Procedures/Proc_PressurizeForEntry".to_string(),
                "ServiceSet/AirlockService/Procedures/Proc_ManualDoorJog".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/Indicators".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/Parameters".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/ActiveElements".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/Control/OperatorCommands/Req".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/Control/OperatorCommands/Rsp".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/Control/RemoteCommands/Req".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/Control/RemoteCommands/Rsp".to_string(),
                "ServiceSet/AirlockService/DataAssemblies/Control/ActiveCommand".to_string(),
                "Simulation".to_string(),
                "FaultInjection".to_string(),
            ],
        }
    }

    fn run_active_command(&mut self, dt_sec: f64) {
        if self.active_command.state != CommandStatusEnum::Running {
            self.mtp_state_machine.transition_active = false;
            return;
        }

        self.update_channel_status(self.active_command.source, CommandStatusEnum::Running, "");

        match self.active_command.command {
            CommandEnum::StartDepressurizeCycle => {
                self.pump_on = true;
                self.vent_valve_dynamics.command_pct = 100.0;
                self.equalize_valve_dynamics.command_pct = 0.0;
                self.pump_current_a = 9.5;
                self.inner_door_target_pct = 0.0;
                self.outer_door_target_pct = 0.0;
                self.inner_door_position_pct =
                    move_towards(self.inner_door_position_pct, 0.0, 40.0, dt_sec);
                self.outer_door_position_pct =
                    move_towards(self.outer_door_position_pct, 0.0, 40.0, dt_sec);

                self.active_command.progress_pct = progress_between(
                    self.active_start_metric,
                    self.active_target_metric,
                    self.pressure_pa,
                    true,
                );
                self.set_active_procedure_progress(self.active_command.progress_pct);

                if self.pressure_pa <= self.outer_unlock_max_pressure_pa {
                    self.outer_lock_engaged = false;
                    self.finish_active(CommandStatusEnum::Complete, "Depressurize complete");
                    self.state_name = "eva_mode".to_string();
                }
            }
            CommandEnum::StartPressurizeCycle => {
                self.pump_on = false;
                self.pump_current_a = 0.0;
                self.vent_valve_dynamics.command_pct = 0.0;
                self.equalize_valve_dynamics.command_pct = 100.0;
                self.inner_door_target_pct = 0.0;
                self.outer_door_target_pct = 0.0;
                self.inner_door_position_pct =
                    move_towards(self.inner_door_position_pct, 0.0, 40.0, dt_sec);
                self.outer_door_position_pct =
                    move_towards(self.outer_door_position_pct, 0.0, 40.0, dt_sec);

                self.active_command.progress_pct = progress_between(
                    self.active_start_metric,
                    self.active_target_metric,
                    self.pressure_pa,
                    false,
                );
                self.set_active_procedure_progress(self.active_command.progress_pct);

                if self.pressure_pa >= self.inner_unlock_min_pressure_pa {
                    self.inner_lock_engaged = false;
                    self.finish_active(CommandStatusEnum::Complete, "Pressurize complete");
                    self.state_name = "ready_for_entry".to_string();
                }
            }
            CommandEnum::SetInnerDoorTargetPct => {
                self.inner_door_position_pct = move_towards(
                    self.inner_door_position_pct,
                    self.inner_door_target_pct,
                    25.0,
                    dt_sec,
                );
                self.active_command.progress_pct = progress_linear(
                    self.active_start_metric,
                    self.active_target_metric,
                    self.inner_door_position_pct,
                );
                self.set_active_procedure_progress(self.active_command.progress_pct);

                if (self.inner_door_position_pct - self.inner_door_target_pct).abs() < 0.1 {
                    self.finish_active(CommandStatusEnum::Complete, "Inner door position reached");
                    self.state_name = "manual_override".to_string();
                }
            }
            CommandEnum::SetOuterDoorTargetPct => {
                self.outer_door_position_pct = move_towards(
                    self.outer_door_position_pct,
                    self.outer_door_target_pct,
                    25.0,
                    dt_sec,
                );
                self.active_command.progress_pct = progress_linear(
                    self.active_start_metric,
                    self.active_target_metric,
                    self.outer_door_position_pct,
                );
                self.set_active_procedure_progress(self.active_command.progress_pct);

                if (self.outer_door_position_pct - self.outer_door_target_pct).abs() < 0.1 {
                    self.finish_active(CommandStatusEnum::Complete, "Outer door position reached");
                    self.state_name = "manual_override".to_string();
                }
            }
            _ => {
                self.finish_active(CommandStatusEnum::Complete, "Command complete");
            }
        }
    }

    fn apply_physics(&mut self, dt_sec: f64) {
        let pressure_hab = 101_325.0;
        let pressure_mars = 700.0;

        self.equalize_valve_dynamics
            .step(&mut self.equalize_valve_pct, dt_sec);
        self.vent_valve_dynamics
            .step(&mut self.vent_valve_pct, dt_sec);

        let f_eq = self
            .equalize_valve_dynamics
            .effective_flow_position_pct(self.equalize_valve_pct)
            / 100.0;
        let f_vent = self
            .vent_valve_dynamics
            .effective_flow_position_pct(self.vent_valve_pct)
            / 100.0;
        let pump_factor = if self.pump_on { 1.0 } else { 0.0 };

        let k_eq = 0.04;
        let k_vent = 0.07;
        let k_pump = 0.15;
        let k_leak = self.leak_rate_nominal;

        let dp_dt = k_eq * f_eq * (pressure_hab - self.pressure_pa)
            + k_vent * f_vent * (pressure_mars - self.pressure_pa)
            - k_pump * pump_factor * self.pressure_pa
            - k_leak * (self.pressure_pa - pressure_mars);

        self.pressure_pa += dp_dt * dt_sec;
        self.pressure_pa = self.pressure_pa.clamp(pressure_mars, 120_000.0);

        let ambient_temperature_k = 210.0 + (f_eq * 83.0);
        self.temperature_k += (ambient_temperature_k - self.temperature_k) * 0.02 * dt_sec;

        let o2_target = if f_eq > 0.01 { 21.0 } else { 1.0 };
        self.o2_percent += (o2_target - self.o2_percent) * 0.04 * dt_sec;
        self.o2_percent = self.o2_percent.clamp(0.1, 22.0);

        if self.pump_on {
            self.pump_current_a = 8.0 + (self.vent_valve_pct / 100.0) * 2.0;
        } else {
            self.pump_current_a = 0.0;
        }
    }

    pub fn set_valve_fault(&mut self, request: ValveFaultUpdateRequest) -> Result<(), String> {
        let dynamics = match request.valve.trim().to_ascii_lowercase().as_str() {
            "equalize" | "equalization" | "equalize_valve" => &mut self.equalize_valve_dynamics,
            "vent" | "vent_valve" => &mut self.vent_valve_dynamics,
            _ => return Err("Unknown valve. Use equalize or vent".to_string()),
        };
        if let Some(value) = request.stiction_breakaway_pct {
            dynamics.stiction_breakaway_pct = value.clamp(0.0, 100.0);
        }
        if let Some(value) = request.sensor_bias_pct {
            dynamics.sensor_bias_pct = value.clamp(-100.0, 100.0);
        }
        if let Some(value) = request.max_travel_rate_pct_per_sec {
            if !value.is_finite() || value <= 0.0 {
                return Err("max_travel_rate_pct_per_sec must be positive".to_string());
            }
            dynamics.max_travel_rate_pct_per_sec = value.min(1_000.0);
        }
        if let Some(value) = request.hard_stuck {
            dynamics.hard_stuck = value;
        }
        if let Some(value) = request.leakage_pct {
            dynamics.leakage_pct = value.clamp(0.0, 100.0);
        }
        self.log_event(
            "WARN",
            "FAULT_INJECTION",
            format!(
                "Valve actuator fault parameters updated for {}",
                request.valve
            ),
        );
        Ok(())
    }

    fn update_alarms(&mut self) {
        self.alarms.high_pressure_alarm_active = self.pressure_pa > self.high_pressure_alarm_pa;
        self.alarms.low_pressure_alarm_active = self.pressure_pa < self.low_pressure_alarm_pa;
        self.alarms.leak_detected = self.leak_rate_nominal > 0.001;
        self.alarms.out_of_spec = self.alarms.high_pressure_alarm_active
            || self.alarms.low_pressure_alarm_active
            || self.alarms.interlock_violation;

        let mut parts = Vec::new();
        if self.alarms.high_pressure_alarm_active {
            parts.push("HIGH_PRESSURE");
        }
        if self.alarms.low_pressure_alarm_active {
            parts.push("LOW_PRESSURE");
        }
        if self.alarms.interlock_violation {
            parts.push("INTERLOCK");
        }
        if self.alarms.leak_detected {
            parts.push("LEAK");
        }

        self.alarms.alarm_summary = if parts.is_empty() {
            "OK".to_string()
        } else {
            parts.join("|")
        };

        self.mtp_runtime.pea_information_label.health_state = if self.alarms.out_of_spec {
            "WARN".to_string()
        } else {
            "OK".to_string()
        };
    }

    fn validate_request(
        &self,
        source: CommandSourceEnum,
        request: &CommandRequestFields,
    ) -> Option<String> {
        match source {
            CommandSourceEnum::OperatorUi if !self.permissions.operator_control_enabled => {
                return Some("Permissions: operator control disabled".to_string());
            }
            CommandSourceEnum::RemoteOpcua if !self.permissions.remote_control_enabled => {
                return Some("Permissions: remote control disabled".to_string());
            }
            _ => {}
        }

        if !self.mtp_modes.command_en && request.command != CommandEnum::AbortCycle {
            return Some(if self.mtp_modes.command_en_reason.is_empty() {
                "Blocked: CommandEn disabled".to_string()
            } else {
                format!("Blocked: {}", self.mtp_modes.command_en_reason)
            });
        }

        if self.active_command.state == CommandStatusEnum::Running
            && request.command != CommandEnum::AbortCycle
        {
            return Some("Busy: command already active".to_string());
        }

        match request.command {
            CommandEnum::UnlockOuterDoor => {
                if self.pressure_pa > self.outer_unlock_max_pressure_pa {
                    return Some("Blocked: pressure too high for outer unlock".to_string());
                }
                if self.inner_door_position_pct > 0.1 {
                    return Some("Interlock: inner door open".to_string());
                }
            }
            CommandEnum::UnlockInnerDoor => {
                if self.pressure_pa < self.inner_unlock_min_pressure_pa {
                    return Some("Blocked: pressure too low for inner unlock".to_string());
                }
                if self.outer_door_position_pct > 0.1 {
                    return Some("Interlock: outer door open".to_string());
                }
            }
            CommandEnum::SetOuterDoorTargetPct => {
                if request.param1 > 0.0 && self.inner_door_position_pct > 0.1 {
                    return Some("Interlock: inner door open".to_string());
                }
                if request.param1 > 0.0 && self.outer_lock_engaged {
                    return Some("Blocked: outer door locked".to_string());
                }
            }
            CommandEnum::SetInnerDoorTargetPct => {
                if request.param1 > 0.0 && self.outer_door_position_pct > 0.1 {
                    return Some("Interlock: outer door open".to_string());
                }
                if request.param1 > 0.0 && self.inner_lock_engaged {
                    return Some("Blocked: inner door locked".to_string());
                }
            }
            CommandEnum::StartDepressurizeCycle | CommandEnum::StartPressurizeCycle => {
                if self.inner_door_position_pct > 0.1 {
                    return Some("Interlock: inner door open".to_string());
                }
                if self.outer_door_position_pct > 0.1 {
                    return Some("Interlock: outer door open".to_string());
                }
            }
            _ => {}
        }

        None
    }

    fn apply_immediate_command(&mut self, command: CommandEnum, param1: f64, _param2: f64) -> bool {
        match command {
            CommandEnum::SetPumpOn => {
                self.pump_on = param1 >= 0.5;
                self.pump_current_a = if self.pump_on { 8.0 } else { 0.0 };
                self.finish_active(CommandStatusEnum::Complete, "Pump state updated");
                true
            }
            CommandEnum::SetEqualizeValvePct => {
                self.equalize_valve_dynamics.command_pct = param1.clamp(0.0, 100.0);
                self.finish_active(CommandStatusEnum::Complete, "Equalize valve updated");
                true
            }
            CommandEnum::SetVentValvePct => {
                self.vent_valve_dynamics.command_pct = param1.clamp(0.0, 100.0);
                self.finish_active(CommandStatusEnum::Complete, "Vent valve updated");
                true
            }
            CommandEnum::LockInnerDoor => {
                self.inner_lock_engaged = true;
                self.finish_active(CommandStatusEnum::Complete, "Inner door locked");
                true
            }
            CommandEnum::UnlockInnerDoor => {
                self.inner_lock_engaged = false;
                self.finish_active(CommandStatusEnum::Complete, "Inner door unlocked");
                true
            }
            CommandEnum::LockOuterDoor => {
                self.outer_lock_engaged = true;
                self.finish_active(CommandStatusEnum::Complete, "Outer door locked");
                true
            }
            CommandEnum::UnlockOuterDoor => {
                self.outer_lock_engaged = false;
                self.finish_active(CommandStatusEnum::Complete, "Outer door unlocked");
                true
            }
            CommandEnum::ResetFaults => {
                self.alarms.interlock_violation = false;
                self.alarms.leak_detected = false;
                self.alarms.out_of_spec = false;
                self.diagnostics.last_rejected_command.clear();
                self.mtp_state_machine.blocking_condition.clear();
                self.finish_active(CommandStatusEnum::Complete, "Faults reset");
                true
            }
            CommandEnum::AbortCycle => {
                self.pump_on = false;
                self.pump_current_a = 0.0;
                self.equalize_valve_dynamics.command_pct = 0.0;
                self.vent_valve_dynamics.command_pct = 0.0;
                self.finish_active(CommandStatusEnum::Aborted, "Abort requested");
                self.state_name = "faulted".to_string();
                true
            }
            _ => false,
        }
    }

    fn finish_active(&mut self, status: CommandStatusEnum, reason: &str) {
        self.active_command.state = status;
        self.active_command.progress_pct = 100.0;
        self.active_command.blocking_condition = reason.to_string();

        self.update_channel_status(self.active_command.source, status, reason);
        self.complete_active_procedure(status, reason);

        self.mtp_state_machine.current_state = match status {
            CommandStatusEnum::Aborted => ServiceState::Aborted,
            CommandStatusEnum::Rejected => ServiceState::Held,
            _ => ServiceState::Completed,
        };
        self.mtp_state_machine.transition_active = false;
        self.mtp_state_machine.blocking_condition = reason.to_string();

        if self.active_command.command == CommandEnum::None {
            self.state_name = "idle".to_string();
        }

        self.log_event(
            if status == CommandStatusEnum::Aborted {
                "WARN"
            } else {
                "INFO"
            },
            self.active_command.source.as_str(),
            format!(
                "{} -> {:?} ({})",
                display_command(self.active_command.command),
                status,
                reason
            ),
        );
    }

    fn update_channel_status(
        &mut self,
        source: CommandSourceEnum,
        status: CommandStatusEnum,
        reason: &str,
    ) {
        let channel = self.channel_mut(source);
        channel.rsp.status = status;
        channel.rsp.reject_reason = reason.to_string();
        channel.rsp.last_update_time_ms = Self::now_ms();
    }

    fn set_channel_response(&mut self, source: CommandSourceEnum, response: CommandResponseFields) {
        let channel = self.channel_mut(source);
        channel.rsp = response;
    }

    fn channel(&self, source: CommandSourceEnum) -> &CommandChannelState {
        match source {
            CommandSourceEnum::OperatorUi => &self.operator_channel,
            CommandSourceEnum::RemoteOpcua => &self.remote_channel,
            CommandSourceEnum::SystemAuto => &self.operator_channel,
        }
    }

    fn channel_mut(&mut self, source: CommandSourceEnum) -> &mut CommandChannelState {
        match source {
            CommandSourceEnum::OperatorUi => &mut self.operator_channel,
            CommandSourceEnum::RemoteOpcua => &mut self.remote_channel,
            CommandSourceEnum::SystemAuto => &mut self.operator_channel,
        }
    }

    fn last_execute_for(&self, source: CommandSourceEnum) -> bool {
        match source {
            CommandSourceEnum::OperatorUi => self.last_operator_execute,
            CommandSourceEnum::RemoteOpcua => self.last_remote_execute,
            CommandSourceEnum::SystemAuto => false,
        }
    }

    fn set_last_execute_for(&mut self, source: CommandSourceEnum, value: bool) {
        match source {
            CommandSourceEnum::OperatorUi => self.last_operator_execute = value,
            CommandSourceEnum::RemoteOpcua => self.last_remote_execute = value,
            CommandSourceEnum::SystemAuto => {}
        }
    }

    fn refresh_client_counts(&mut self) {
        let web_clients = self.diagnostics.connected_clients.len() as u32;
        self.diagnostics.connected_client_summary = format!(
            "opcua_sessions={}, opcua_subscriptions={}, web_sessions={web_clients}",
            self.diagnostics.connected_client_count, self.diagnostics.subscription_count
        );
    }

    fn set_active_procedure_for_command(&mut self, command: CommandEnum) {
        self.mtp_runtime.service_information.active_procedure =
            procedure_for_command(command).unwrap_or("None").to_string();

        for procedure in &mut self.mtp_runtime.procedures {
            if Some(procedure.name.as_str()) == procedure_for_command(command) {
                procedure.proc_state = "running".to_string();
                procedure.progress_pct = 0.0;
                procedure.last_result.clear();
            }
        }
    }

    fn set_active_procedure_progress(&mut self, progress_pct: f64) {
        let active_name = self
            .mtp_runtime
            .service_information
            .active_procedure
            .clone();

        if active_name == "None" {
            return;
        }

        if let Some(procedure) = self
            .mtp_runtime
            .procedures
            .iter_mut()
            .find(|entry| entry.name == active_name)
        {
            procedure.progress_pct = progress_pct.clamp(0.0, 100.0);
            procedure.proc_state = "running".to_string();
        }
    }

    fn complete_active_procedure(&mut self, status: CommandStatusEnum, reason: &str) {
        let active_name = self
            .mtp_runtime
            .service_information
            .active_procedure
            .clone();

        if active_name != "None" {
            if let Some(procedure) = self
                .mtp_runtime
                .procedures
                .iter_mut()
                .find(|entry| entry.name == active_name)
            {
                procedure.progress_pct = 100.0;
                procedure.proc_state = if status == CommandStatusEnum::Aborted {
                    "aborted".to_string()
                } else {
                    "complete".to_string()
                };
                procedure.last_result = if reason.is_empty() {
                    "OK".to_string()
                } else {
                    reason.to_string()
                };
            }
        }

        self.mtp_runtime.service_information.active_procedure = "None".to_string();
    }

    fn log_event(&mut self, severity: &str, source: impl Into<String>, message: impl Into<String>) {
        self.event_log.push(EventEntry {
            timestamp_ms: Self::now_ms(),
            severity: severity.to_string(),
            source: source.into(),
            message: message.into(),
        });

        if self.event_log.len() > 120 {
            let drain = self.event_log.len() - 120;
            self.event_log.drain(0..drain);
        }
    }
}

fn move_towards(current: f64, target: f64, speed_per_sec: f64, dt_sec: f64) -> f64 {
    let delta = speed_per_sec * dt_sec;
    if (current - target).abs() <= delta {
        target
    } else if current < target {
        current + delta
    } else {
        current - delta
    }
}

fn progress_linear(start: f64, target: f64, current: f64) -> f64 {
    let span = (target - start).abs();
    if span < f64::EPSILON {
        100.0
    } else {
        ((current - start).abs() / span * 100.0).clamp(0.0, 100.0)
    }
}

fn progress_between(start: f64, target: f64, current: f64, decreasing: bool) -> f64 {
    if (target - start).abs() < f64::EPSILON {
        return 100.0;
    }

    if decreasing {
        ((start - current) / (start - target) * 100.0).clamp(0.0, 100.0)
    } else {
        ((current - start) / (target - start) * 100.0).clamp(0.0, 100.0)
    }
}

fn display_command(command: CommandEnum) -> &'static str {
    match command {
        CommandEnum::None => "NONE",
        CommandEnum::StartDepressurizeCycle => "START_DEPRESSURIZE_CYCLE",
        CommandEnum::StartPressurizeCycle => "START_PRESSURIZE_CYCLE",
        CommandEnum::AbortCycle => "ABORT_CYCLE",
        CommandEnum::ResetFaults => "RESET_FAULTS",
        CommandEnum::SetPumpOn => "SET_PUMP_ON",
        CommandEnum::SetEqualizeValvePct => "SET_EQUALIZE_VALVE_PCT",
        CommandEnum::SetVentValvePct => "SET_VENT_VALVE_PCT",
        CommandEnum::SetInnerDoorTargetPct => "SET_INNER_DOOR_TARGET_PCT",
        CommandEnum::SetOuterDoorTargetPct => "SET_OUTER_DOOR_TARGET_PCT",
        CommandEnum::LockInnerDoor => "LOCK_INNER_DOOR",
        CommandEnum::UnlockInnerDoor => "UNLOCK_INNER_DOOR",
        CommandEnum::LockOuterDoor => "LOCK_OUTER_DOOR",
        CommandEnum::UnlockOuterDoor => "UNLOCK_OUTER_DOOR",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulation() -> Simulation {
        Simulation::new("NONE".to_string(), "opc.tcp://test".to_string())
    }

    #[test]
    fn valve_position_is_rate_limited_instead_of_instantaneous() {
        let mut sim = simulation();
        sim.set_valve_fault(ValveFaultUpdateRequest {
            valve: "vent".to_string(),
            stiction_breakaway_pct: Some(0.0),
            sensor_bias_pct: Some(0.0),
            max_travel_rate_pct_per_sec: Some(10.0),
            hard_stuck: Some(false),
            leakage_pct: Some(0.0),
        })
        .unwrap();
        sim.vent_valve_dynamics.command_pct = 100.0;
        sim.step(1.0);
        let snapshot = sim.snapshot();
        assert!((snapshot.vent_valve_pct - 10.0).abs() < 1.0e-9);
        assert!((snapshot.vent_valve_command_pct - 100.0).abs() < 1.0e-9);
        assert!((snapshot.vent_valve_residual_pct - 90.0).abs() < 1.0e-9);
    }

    #[test]
    fn stiction_holds_until_command_exceeds_breakaway() {
        let mut sim = simulation();
        sim.set_valve_fault(ValveFaultUpdateRequest {
            valve: "equalize".to_string(),
            stiction_breakaway_pct: Some(20.0),
            sensor_bias_pct: None,
            max_travel_rate_pct_per_sec: Some(100.0),
            hard_stuck: None,
            leakage_pct: None,
        })
        .unwrap();
        sim.equalize_valve_dynamics.command_pct = 10.0;
        sim.step(1.0);
        assert_eq!(sim.snapshot().equalize_valve_pct, 0.0);
        assert!(sim.snapshot().equalize_valve_stiction_active);

        sim.equalize_valve_dynamics.command_pct = 30.0;
        sim.step(0.1);
        assert!(sim.snapshot().equalize_valve_pct > 0.0);
        assert!(!sim.snapshot().equalize_valve_stiction_active);
    }

    #[test]
    fn sensor_bias_changes_observation_not_physical_flow_position() {
        let mut sim = simulation();
        sim.set_valve_fault(ValveFaultUpdateRequest {
            valve: "vent".to_string(),
            stiction_breakaway_pct: None,
            sensor_bias_pct: Some(7.5),
            max_travel_rate_pct_per_sec: None,
            hard_stuck: Some(true),
            leakage_pct: None,
        })
        .unwrap();
        sim.step(0.05);
        let snapshot = sim.snapshot();
        assert_eq!(snapshot.vent_valve_pct, 0.0);
        assert_eq!(snapshot.vent_valve_sensed_pct, 7.5);
        assert_eq!(snapshot.vent_valve_residual_pct, -7.5);
    }

    #[test]
    fn actuator_fault_state_survives_checkpoint_serialization() {
        let mut sim = simulation();
        sim.set_valve_fault(ValveFaultUpdateRequest {
            valve: "equalize".to_string(),
            stiction_breakaway_pct: Some(12.0),
            sensor_bias_pct: Some(-3.0),
            max_travel_rate_pct_per_sec: Some(8.0),
            hard_stuck: Some(true),
            leakage_pct: Some(1.5),
        })
        .unwrap();
        sim.equalize_valve_dynamics.command_pct = 75.0;
        sim.step(0.05);
        let restored: Simulation =
            serde_json::from_slice(&serde_json::to_vec(&sim).unwrap()).unwrap();
        assert_eq!(restored.equalize_valve_dynamics.command_pct, 75.0);
        assert_eq!(
            restored.equalize_valve_dynamics.stiction_breakaway_pct,
            12.0
        );
        assert_eq!(restored.equalize_valve_dynamics.sensor_bias_pct, -3.0);
        assert!(restored.equalize_valve_dynamics.hard_stuck);
        assert_eq!(restored.equalize_valve_dynamics.leakage_pct, 1.5);
    }
}

fn procedure_for_command(command: CommandEnum) -> Option<&'static str> {
    match command {
        CommandEnum::StartDepressurizeCycle => Some(PROC_DEPRESSURIZE),
        CommandEnum::StartPressurizeCycle => Some(PROC_PRESSURIZE),
        CommandEnum::SetInnerDoorTargetPct | CommandEnum::SetOuterDoorTargetPct => {
            Some(PROC_MANUAL_JOG)
        }
        _ => None,
    }
}
