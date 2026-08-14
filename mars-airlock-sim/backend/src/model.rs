use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandEnum {
    None,
    StartDepressurizeCycle,
    StartPressurizeCycle,
    AbortCycle,
    ResetFaults,
    SetPumpOn,
    SetEqualizeValvePct,
    SetVentValvePct,
    SetInnerDoorTargetPct,
    SetOuterDoorTargetPct,
    LockInnerDoor,
    UnlockInnerDoor,
    LockOuterDoor,
    UnlockOuterDoor,
}

impl Default for CommandEnum {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandStatusEnum {
    Idle,
    Accepted,
    Rejected,
    Running,
    Complete,
    Aborted,
}

impl Default for CommandStatusEnum {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandSourceEnum {
    OperatorUi,
    RemoteOpcua,
    SystemAuto,
}

impl CommandSourceEnum {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OperatorUi => "OPERATOR_UI",
            Self::RemoteOpcua => "REMOTE_OPCUA",
            Self::SystemAuto => "SYSTEM_AUTO",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationMode {
    Off,
    Auto,
    Manual,
    Maint,
}

impl Default for OperationMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Idle,
    Starting,
    Execute,
    Completing,
    Completed,
    Pausing,
    Paused,
    Resuming,
    Holding,
    Held,
    Unholding,
    Stopping,
    Stopped,
    Aborting,
    Aborted,
    Resetting,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequestFields {
    pub sequence_id: u32,
    pub command: CommandEnum,
    pub param1: f64,
    pub param2: f64,
    pub execute: bool,
}

impl Default for CommandRequestFields {
    fn default() -> Self {
        Self {
            sequence_id: 0,
            command: CommandEnum::None,
            param1: 0.0,
            param2: 0.0,
            execute: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponseFields {
    pub ack_sequence_id: u32,
    pub status: CommandStatusEnum,
    pub reject_reason: String,
    pub last_update_time_ms: u64,
}

impl Default for CommandResponseFields {
    fn default() -> Self {
        Self {
            ack_sequence_id: 0,
            status: CommandStatusEnum::Idle,
            reject_reason: String::new(),
            last_update_time_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandChannelState {
    pub req: CommandRequestFields,
    pub rsp: CommandResponseFields,
    pub last_sequence_processed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCommand {
    pub command: CommandEnum,
    pub source: CommandSourceEnum,
    pub sequence_id: u32,
    pub param1: f64,
    pub param2: f64,
    pub state: CommandStatusEnum,
    pub progress_pct: f64,
    pub blocking_condition: String,
    pub start_time_ms: u64,
    pub last_update_time_ms: u64,
}

impl Default for ActiveCommand {
    fn default() -> Self {
        Self {
            command: CommandEnum::None,
            source: CommandSourceEnum::SystemAuto,
            sequence_id: 0,
            param1: 0.0,
            param2: 0.0,
            state: CommandStatusEnum::Idle,
            progress_pct: 0.0,
            blocking_condition: String::new(),
            start_time_ms: 0,
            last_update_time_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permissions {
    pub operator_control_enabled: bool,
    pub remote_control_enabled: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            operator_control_enabled: true,
            remote_control_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlarmState {
    pub high_pressure_alarm_active: bool,
    pub low_pressure_alarm_active: bool,
    pub interlock_violation: bool,
    pub leak_detected: bool,
    pub out_of_spec: bool,
    pub alarm_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSession {
    pub session_id: u64,
    pub application_name: String,
    pub session_name: String,
    pub remote_address: String,
    pub last_activity_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsState {
    pub endpoint_url: String,
    pub security_modes_enabled: Vec<String>,
    pub active_security_mode: String,
    pub server_start_time_ms: u64,
    pub server_uptime_sec: f64,
    pub connected_client_count: u32,
    pub connected_client_summary: String,
    pub connected_clients: Vec<ClientSession>,
    pub subscription_count: u32,
    pub publishing_rate_hz: f64,
    pub last_rejected_command: String,
    pub last_error: String,
}

impl Default for DiagnosticsState {
    fn default() -> Self {
        Self {
            endpoint_url: "opc.tcp://127.0.0.1:4841/mars-airlock".to_string(),
            security_modes_enabled: vec!["NONE".to_string(), "BASIC256SHA256".to_string()],
            active_security_mode: "NONE".to_string(),
            server_start_time_ms: 0,
            server_uptime_sec: 0.0,
            connected_client_count: 0,
            connected_client_summary: "none".to_string(),
            connected_clients: Vec::new(),
            subscription_count: 0,
            publishing_rate_hz: 10.0,
            last_rejected_command: String::new(),
            last_error: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpModes {
    pub operation_mode: OperationMode,
    pub source_mode: CommandSourceEnum,
    pub command_en: bool,
    pub command_en_reason: String,
}

impl Default for MtpModes {
    fn default() -> Self {
        Self {
            operation_mode: OperationMode::Auto,
            source_mode: CommandSourceEnum::SystemAuto,
            command_en: true,
            command_en_reason: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpStateMachine {
    pub current_state: ServiceState,
    pub time_in_state_sec: f64,
    pub transition_active: bool,
    pub blocking_condition: String,
}

impl Default for MtpStateMachine {
    fn default() -> Self {
        Self {
            current_state: ServiceState::Idle,
            time_in_state_sec: 0.0,
            transition_active: false,
            blocking_condition: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub timestamp_ms: u64,
    pub severity: String,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeaInformationLabel {
    pub tag_name: String,
    pub module_name: String,
    pub manufacturer: String,
    pub serial_number: String,
    pub hardware_revision: String,
    pub software_revision: String,
    pub mtp_version: String,
    pub opcuaruntime_version: String,
    pub build_timestamp_ms: u64,
    pub documentation_uri: String,
    pub health_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInformation {
    pub service_name: String,
    pub service_id: u32,
    pub service_revision: String,
    pub active_procedure: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureRuntime {
    pub id: u32,
    pub name: String,
    pub proc_state: String,
    pub progress_pct: f64,
    pub last_result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpRuntime {
    pub pea_information_label: PeaInformationLabel,
    pub service_information: ServiceInformation,
    pub procedures: Vec<ProcedureRuntime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub pressure_pa: f64,
    pub temperature_k: f64,
    pub o2_percent: f64,
    pub leak_rate_nominal: f64,
    pub fixed_timestep_sec: f64,
    pub outer_unlock_max_pressure_pa: f64,
    pub inner_unlock_min_pressure_pa: f64,
    pub high_pressure_alarm_pa: f64,
    pub low_pressure_alarm_pa: f64,
    pub inner_door_position_pct: f64,
    pub outer_door_position_pct: f64,
    pub inner_lock_engaged: bool,
    pub outer_lock_engaged: bool,
    pub equalize_valve_pct: f64,
    pub vent_valve_pct: f64,
    pub pump_on: bool,
    pub pump_current_a: f64,
    pub state_name: String,
    pub active_command: ActiveCommand,
    pub alarms: AlarmState,
    pub permissions: Permissions,
    pub diagnostics: DiagnosticsState,
    pub mtp_modes: MtpModes,
    pub mtp_state_machine: MtpStateMachine,
    pub mtp_runtime: MtpRuntime,
    pub event_log: Vec<EventEntry>,
    pub operator_channel: CommandChannelState,
    pub remote_channel: CommandChannelState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpTreeResponse {
    pub namespace: String,
    pub root_path: String,
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityProfileRequest {
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsUpdateRequest {
    pub operator_control_enabled: Option<bool>,
    pub remote_control_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpModesUpdateRequest {
    pub operation_mode: Option<OperationMode>,
    pub command_en: Option<bool>,
    pub command_en_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakRateUpdateRequest {
    pub leak_rate_nominal: f64,
}

// ============================================================================
// MTP Compliance: Parameter Type System (VDI/VDE/NAMUR 2658-3)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ParameterCategory {
    ConfigurationParameter,
    ProcedureParameter,
    ReportValue,
    ProcessValueIn,
    ProcessValueOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceParameter {
    pub name: String,
    pub category: ParameterCategory,
    pub data_type: String, // e.g., "xs:double", "xs:unsignedInt", "xs:string", "xs:boolean"
    pub unit: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
    pub current_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceProcedure {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub input_parameters: Vec<String>, // Parameter names that are inputs
    pub output_parameters: Vec<String>, // Parameter names that are outputs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub id: u32,
    pub name: String,
    pub parameters: Vec<ServiceParameter>,
    pub procedures: Vec<ServiceProcedure>,
}

// ============================================================================
// MTP Compliance: Procedure Calling Convention (VDI/VDE/NAMUR 2658-1 RFC 4.2.1)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureRequest {
    pub request_id: u32,
    pub procedure_id: u32,
    pub procedure_name: String,
    pub service_name: String,
    pub pea_id: String,
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
    pub requested_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProcedureState {
    Idle,
    Running,
    Done,
    Error,
    Blocked,
}

impl Default for ProcedureState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpProcedureRuntime {
    pub current_request_id: u32,
    pub state: ProcedureState,
    pub progress_pct: f64,
    pub result: std::collections::HashMap<String, serde_json::Value>,
    pub error_message: Option<String>,
    pub started_at_ms: u64,
    pub completed_at_ms: u64,
}

impl Default for MtpProcedureRuntime {
    fn default() -> Self {
        Self {
            current_request_id: 0,
            state: ProcedureState::Idle,
            progress_pct: 0.0,
            result: std::collections::HashMap::new(),
            error_message: None,
            started_at_ms: 0,
            completed_at_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureRequestInput {
    pub request_id: u32,
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureStatusResponse {
    pub request_id: u32,
    pub current_request_id: u32,
    pub state: ProcedureState,
    pub progress_pct: f64,
    pub result: std::collections::HashMap<String, serde_json::Value>,
    pub error_message: Option<String>,
}
