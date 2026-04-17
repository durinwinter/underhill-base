const state = {
  snapshot: null,
  sequenceId: 1,
  wsConnected: false,
  ws: null,
  mainScene: null,
  controlsDirtyUntilMs: 0,
  activeModals: new Set(),
  subsystemViews: {
    eclss: { descriptor: null, operator: null },
    sabatier: { descriptor: null, operator: null },
  },
};
const LAYOUT_DEBUG_STORAGE_KEY = "underhill.layoutDebug";

const SUBSYSTEMS = {
  eclss: { peaId: "ECLSS-PEA-001", serviceTag: "EclssService" },
  sabatier: { peaId: "SABATIER-PEA-001", serviceTag: "SabatierService" },
};

const AIRLOCK_UI_COMMANDS = [
  { label: "Depressurize", command: "START_DEPRESSURIZE_CYCLE", param1: 0, param2: 0, selector: 'button[data-command="START_DEPRESSURIZE_CYCLE"]' },
  { label: "Pressurize", command: "START_PRESSURIZE_CYCLE", param1: 0, param2: 0, selector: 'button[data-command="START_PRESSURIZE_CYCLE"]' },
  { label: "Abort", command: "ABORT_CYCLE", param1: 0, param2: 0, selector: 'button[data-command="ABORT_CYCLE"]' },
  { label: "Lock Inner Door", command: "LOCK_INNER_DOOR", param1: 0, param2: 0, selector: "#cmd-lock-inner" },
  { label: "Unlock Inner Door", command: "UNLOCK_INNER_DOOR", param1: 0, param2: 0, selector: "#cmd-unlock-inner" },
  { label: "Open Inner Door", command: "SET_INNER_DOOR_TARGET_PCT", param1: 100, param2: 0, selector: "#cmd-open-inner" },
  { label: "Close Inner Door", command: "SET_INNER_DOOR_TARGET_PCT", param1: 0, param2: 0, selector: "#cmd-close-inner" },
  { label: "Lock Outer Door", command: "LOCK_OUTER_DOOR", param1: 0, param2: 0, selector: "#cmd-lock-outer" },
  { label: "Unlock Outer Door", command: "UNLOCK_OUTER_DOOR", param1: 0, param2: 0, selector: "#cmd-unlock-outer" },
  { label: "Open Outer Door", command: "SET_OUTER_DOOR_TARGET_PCT", param1: 100, param2: 0, selector: "#cmd-open-outer" },
  { label: "Close Outer Door", command: "SET_OUTER_DOOR_TARGET_PCT", param1: 0, param2: 0, selector: "#cmd-close-outer" },
];

const ECLSS_UI_COMMANDS = [
  { label: "Start CO2 Scrubber", command: "START_CO2_SCRUBBER_BED_A", selector: "#eclss-cmd-start-scrubber" },
  { label: "Stop CO2 Scrubber", command: "STOP_CO2_SCRUBBER_BED_A", selector: "#eclss-cmd-stop-scrubber" },
  { label: "Start Electrolyzer", command: "START_ELECTROLYZER_STACK", selector: "#eclss-cmd-start-electrolyzer" },
  { label: "Stop Electrolyzer", command: "STOP_ELECTROLYZER_STACK", selector: "#eclss-cmd-stop-electrolyzer" },
  { label: "Start Water Processor", command: "START_WATER_PROCESSOR", selector: "#eclss-cmd-start-water" },
  { label: "Stop Water Processor", command: "STOP_WATER_PROCESSOR", selector: "#eclss-cmd-stop-water" },
  { label: "Isolate Branch", command: "ISOLATE_ECLSS_BRANCH", selector: "#eclss-cmd-isolate" },
  { label: "Restore Branch", command: "RESTORE_ECLSS_BRANCH", selector: "#eclss-cmd-restore" },
  { label: "Enable Safe Haven", command: "ENABLE_SAFE_HAVEN", selector: "#eclss-cmd-safehaven-on" },
  { label: "Disable Safe Haven", command: "DISABLE_SAFE_HAVEN", selector: "#eclss-cmd-safehaven-off" },
  { label: "Set CO2 Flow", command: "SET_CO2_REMOVAL_FLOW_RATE", selector: "#eclss-apply-co2-flow", inputSelector: "#eclss-set-co2-flow" },
  { label: "Set O2 Generation", command: "SET_OXYGEN_GENERATION_RATE", selector: "#eclss-apply-o2-rate", inputSelector: "#eclss-set-o2-rate" },
  { label: "Set Water Recovery", command: "SET_WATER_RECOVERY_TARGET", selector: "#eclss-apply-water-target", inputSelector: "#eclss-set-water-target" },
  { label: "Set Humidity Target", command: "SET_CABIN_HUMIDITY_TARGET", selector: "#eclss-apply-humidity-target", inputSelector: "#eclss-set-humidity-target" },
  { label: "Set Pressure Target", command: "SET_CABIN_PRESSURE_TARGET", selector: "#eclss-apply-pressure-target", inputSelector: "#eclss-set-pressure-target" },
];

const SABATIER_UI_COMMANDS = [
  { label: "Start Methanation", command: "START_METHANATION", selector: "#sabatier-cmd-start-methanation" },
  { label: "Stop Methanation", command: "STOP_METHANATION", selector: "#sabatier-cmd-stop-methanation" },
  { label: "Start Feed Conditioning", command: "START_FEED_CONDITIONING", selector: "#sabatier-cmd-start-feed-conditioning" },
  { label: "Stop Feed Conditioning", command: "STOP_FEED_CONDITIONING", selector: "#sabatier-cmd-stop-feed-conditioning" },
  { label: "Start H2 Recovery", command: "START_HYDROGEN_RECOVERY", selector: "#sabatier-cmd-start-h2-recovery" },
  { label: "Stop H2 Recovery", command: "STOP_HYDROGEN_RECOVERY", selector: "#sabatier-cmd-stop-h2-recovery" },
  { label: "Start Catalyst Regen", command: "START_CATALYST_REGEN", selector: "#sabatier-cmd-start-regen" },
  { label: "Stop Catalyst Regen", command: "STOP_CATALYST_REGEN", selector: "#sabatier-cmd-stop-regen" },
  { label: "Emergency Vent", command: "EXECUTE_EMERGENCY_VENT", selector: "#sabatier-cmd-emergency-vent" },
  { label: "Safe Shutdown", command: "EXECUTE_SAFE_SHUTDOWN", selector: "#sabatier-cmd-safe-shutdown" },
  { label: "Set CO2 Valve", command: "SET_CO2_FEED_VALVE_PCT", selector: "#sabatier-apply-co2-valve", inputSelector: "#sabatier-set-co2-valve" },
  { label: "Set H2 Valve", command: "SET_H2_FEED_VALVE_PCT", selector: "#sabatier-apply-h2-valve", inputSelector: "#sabatier-set-h2-valve" },
  { label: "Set Reactor Temp", command: "SET_REACTOR_TEMP_TARGET_C", selector: "#sabatier-apply-temp-target", inputSelector: "#sabatier-set-temp-target" },
  { label: "Set Reactor Pressure", command: "SET_REACTOR_PRESSURE_TARGET_BAR", selector: "#sabatier-apply-pressure-target", inputSelector: "#sabatier-set-pressure-target" },
];

const dom = {
  modalTriggers: document.querySelectorAll(".hmi-trigger"),
  modals: document.querySelectorAll(".subsystem-modal"),
  closeButtons: document.querySelectorAll(".close-modal"),

  // Base HMI
  baseAirlockState: document.querySelector("#base-airlock-state"),
  baseEclssState: document.querySelector("#base-eclss-state"),
  baseSabatierState: document.querySelector("#base-sabatier-state"),
  stateName: document.querySelector("#state-name"),
  activeCommand: document.querySelector("#active-command"),
  activeProgress: document.querySelector("#active-progress"),
  modeCommandEn: document.querySelector("#mode-command-en"),
  modeCommandEnReason: document.querySelector("#mode-command-en-reason"),
  modeApplyEn: document.querySelector("#mode-apply-en"),
  modeApplyEnReason: document.querySelector("#mode-apply-en-reason"),
  blockingCondition: document.querySelector("#blocking-condition"),
  serverUptime: document.querySelector("#server-uptime"),
  lastReject: document.querySelector("#last-reject"),
  lastError: document.querySelector("#last-error"),
  sessionList: document.querySelector("#session-list"),

  // Airlock HMI
  gPressure: document.querySelector("#g-pressure"),
  vPressure: document.querySelector("#v-pressure"),
  gO2: document.querySelector("#g-o2"),
  vO2: document.querySelector("#v-o2"),
  equalizeValve: document.querySelector("#equalize-valve-icon"),
  ventValve: document.querySelector("#vent-valve-icon"),
  pump: document.querySelector("#pump-icon"),
  eventLog: document.querySelector("#event-log"),
  airlockStatusMsg: document.querySelector("#airlock-status-msg"),
  cmdCurrentState: document.querySelector("#cmd-current-state"),
  cmdCurrentActive: document.querySelector("#cmd-current-active"),
  cmdBlockedReason: document.querySelector("#cmd-blocked-reason"),
  cmdAllowedList: document.querySelector("#cmd-allowed-list"),

  // ECLSS HMI
  eclssGPressure: document.querySelector("#eclss-g-pressure"),
  eclssGCO2: document.querySelector("#eclss-g-co2"),
  eclssVPressure: document.querySelector("#eclss-v-pressure"),
  eclssVCO2: document.querySelector("#eclss-v-co2"),
  eclssVWaterRecov: document.querySelector("#eclss-v-water-recovery"),
  eclssVO2Gen: document.querySelector("#eclss-v-o2-gen"),
  eclssVCO2Cap: document.querySelector("#eclss-v-co2-capture"),
  eclssVHumid: document.querySelector("#eclss-v-humidity"),
  eclssCmdCurrentState: document.querySelector("#eclss-cmd-current-state"),
  eclssCmdActiveProcedure: document.querySelector("#eclss-cmd-active-procedure"),
  eclssCmdLastCommand: document.querySelector("#eclss-cmd-last-command"),
  eclssCmdLastStatus: document.querySelector("#eclss-cmd-last-status"),
  eclssCmdLastReason: document.querySelector("#eclss-cmd-last-reason"),
  eclssCmdAllowedList: document.querySelector("#eclss-cmd-allowed-list"),

  // Sabatier HMI
  sabatierGTemp: document.querySelector("#sabatier-g-temp"),
  sabatierGCH4Feed: document.querySelector("#sabatier-g-co2-feed"),
  sabatierVTemp: document.querySelector("#sabatier-v-temp"),
  sabatierVCo2Feed: document.querySelector("#sabatier-v-co2-feed"),
  sabatierVPressure: document.querySelector("#sabatier-v-pressure"),
  sabatierVH2Feed: document.querySelector("#sabatier-v-h2-feed"),
  sabatierVEfficiency: document.querySelector("#sabatier-v-efficiency"),
  sabatierVMethane: document.querySelector("#sabatier-v-methane"),
  sabatierCmdCurrentState: document.querySelector("#sabatier-cmd-current-state"),
  sabatierCmdActiveProcedure: document.querySelector("#sabatier-cmd-active-procedure"),
  sabatierCmdLastCommand: document.querySelector("#sabatier-cmd-last-command"),
  sabatierCmdLastStatus: document.querySelector("#sabatier-cmd-last-status"),
  sabatierCmdLastReason: document.querySelector("#sabatier-cmd-last-reason"),
  sabatierCmdAllowedList: document.querySelector("#sabatier-cmd-allowed-list"),

  threeWrap: document.querySelector("#three-wrap"),
  pill: document.querySelector("#connection-pill"),
  securitySelect: document.querySelector("#security-select"),
  layoutDebugToggle: document.querySelector("#layout-debug-toggle"),
};

async function bootstrap() {
  bindEvents();
  await initThree();
  await Promise.all([
    fetchSnapshot(),
    refreshSubsystemOperatorState("eclss"),
    refreshSubsystemOperatorState("sabatier"),
  ]);
  connectWebSocket();
  setInterval(() => {
    if (!state.wsConnected) {
      fetchSnapshot().catch(() => undefined);
    }
  }, 1000);
  setInterval(() => {
    refreshSubsystemOperatorState("eclss").catch(() => undefined);
    refreshSubsystemOperatorState("sabatier").catch(() => undefined);
  }, 2000);
}

function bindEvents() {
  // Modal toggling
  dom.modalTriggers.forEach(btn => {
    btn.addEventListener("click", () => {
      const modalId = btn.getAttribute("data-modal");
      toggleModal(modalId);
    });
  });

  dom.closeButtons.forEach(btn => {
    btn.addEventListener("click", () => {
      const modal = btn.closest(".subsystem-modal");
      toggleModal(modal.id, false);
    });
  });

  // Commands
  document.querySelectorAll("button[data-command]").forEach((button) => {
    button.addEventListener("click", async () => {
      const command = button.getAttribute("data-command");
      await sendOperatorCommand(command, 0, 0);
    });
  });

  // Door specific
  document.querySelector("#cmd-open-inner")?.addEventListener("click", () => setDoorTarget("inner", 100));
  document.querySelector("#cmd-close-inner")?.addEventListener("click", () => setDoorTarget("inner", 0));
  document.querySelector("#cmd-open-outer")?.addEventListener("click", () => setDoorTarget("outer", 100));
  document.querySelector("#cmd-close-outer")?.addEventListener("click", () => setDoorTarget("outer", 0));

  document.querySelector("#cmd-lock-inner")?.addEventListener("click", () => void sendOperatorCommand("LOCK_INNER_DOOR", 0, 0));
  document.querySelector("#cmd-unlock-inner")?.addEventListener("click", () => void sendOperatorCommand("UNLOCK_INNER_DOOR", 0, 0));
  document.querySelector("#cmd-lock-outer")?.addEventListener("click", () => void sendOperatorCommand("LOCK_OUTER_DOOR", 0, 0));
  document.querySelector("#cmd-unlock-outer")?.addEventListener("click", () => void sendOperatorCommand("UNLOCK_OUTER_DOOR", 0, 0));

  // Subsystem buttons
  document.querySelector("#eclss-deploy")?.addEventListener("click", () => void sendSubsystemLifecycle("eclss", "deploy"));
  document.querySelector("#eclss-start")?.addEventListener("click", () => void sendSubsystemLifecycle("eclss", "start"));
  document.querySelector("#eclss-stop")?.addEventListener("click", () => void sendSubsystemLifecycle("eclss", "stop"));

  document.querySelector("#sabatier-deploy")?.addEventListener("click", () => void sendSubsystemLifecycle("sabatier", "deploy"));
  document.querySelector("#sabatier-start")?.addEventListener("click", () => void sendSubsystemLifecycle("sabatier", "start"));
  document.querySelector("#sabatier-stop")?.addEventListener("click", () => void sendSubsystemLifecycle("sabatier", "stop"));

  bindSubsystemAdvancedButtons("eclss", ECLSS_UI_COMMANDS);
  bindSubsystemAdvancedButtons("sabatier", SABATIER_UI_COMMANDS);

  dom.securitySelect.addEventListener("change", async () => {
    await fetch("/api/security/profile", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ profile: dom.securitySelect.value }),
    });
  });

  dom.layoutDebugToggle?.addEventListener("change", () => {
    const enabled = !!dom.layoutDebugToggle.checked;
    try {
      window.localStorage.setItem(LAYOUT_DEBUG_STORAGE_KEY, enabled ? "1" : "0");
    } catch (_) {
      // ignore storage errors
    }
    state.mainScene?.setLayoutDebug?.(enabled);
  });
}

function bindSubsystemAdvancedButtons(system, commandDefs) {
  commandDefs.forEach((def) => {
    const button = document.querySelector(def.selector);
    if (!button) return;
    button.addEventListener("click", async () => {
      const input = def.inputSelector ? document.querySelector(def.inputSelector) : null;
      const param1 = input ? Number(input.value || 0) : (def.param1 || 0);
      const param2 = def.param2 || 0;
      await sendSubsystemAdvancedCommand(system, def.command, param1, param2);
    });
  });
}

function toggleModal(id, force) {
  const modal = document.getElementById(id);
  if (!modal) return;
  const isShowing = force !== undefined ? force : !modal.classList.contains("active");

  if (isShowing) {
    // Single-active-modal behavior: opening one panel closes the others.
    dom.modals.forEach((otherModal) => {
      if (otherModal.id !== id) {
        otherModal.classList.remove("active");
        state.activeModals.delete(otherModal.id);
      }
    });
    modal.classList.add("active");
    modal.scrollTop = 0;
    state.activeModals.add(id);
  } else {
    modal.classList.remove("active");
    state.activeModals.delete(id);
  }
}

async function fetchSnapshot() {
  try {
    const res = await fetch("/api/snapshot");
    if (!res.ok) return;
    const snapshot = await res.json();
    applySnapshot(snapshot);
  } catch (e) {
    // silence
  }
}

function connectWebSocket() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const wsUrl = `${protocol}//${window.location.host}/ws`;
  state.ws = new WebSocket(wsUrl);

  state.ws.onopen = () => {
    state.wsConnected = true;
    dom.pill.textContent = "live";
    dom.pill.className = "pill active";
  };

  state.ws.onclose = () => {
    state.wsConnected = false;
    dom.pill.textContent = "offline";
    dom.pill.className = "pill";
    setTimeout(connectWebSocket, 2000);
  };

  state.ws.onmessage = (evt) => {
    try {
      const snapshot = JSON.parse(evt.data);
      applySnapshot(snapshot);
    } catch (e) {
      console.error("WS error:", e);
    }
  };
}

function applySnapshot(snapshot) {
  state.snapshot = snapshot;
  if (state.mainScene) state.mainScene.update(snapshot);

  // Handle Alarm Flashing on Modals
  const hasAlarm = snapshot.alarms.high_pressure_alarm_active ||
    snapshot.alarms.low_pressure_alarm_active ||
    snapshot.alarms.interlock_violation ||
    snapshot.alarms.leak_detected ||
    snapshot.alarms.out_of_spec;

  document.querySelectorAll(".subsystem-modal").forEach(modal => {
    if (hasAlarm) {
      modal.classList.add("alarm-flash");
    } else {
      modal.classList.remove("alarm-flash");
    }
  });

  // Update Base HMI
  if (dom.baseAirlockState) dom.baseAirlockState.textContent = snapshot.state_name;
  if (dom.baseEclssState) dom.baseEclssState.textContent = snapshot.eclss_o2_percent != null ? "online" : "offline";
  if (dom.baseSabatierState) dom.baseSabatierState.textContent = snapshot.sabatier_reactor_temp_c != null ? "online" : "offline";
  if (dom.activeCommand) dom.activeCommand.textContent = snapshot.active_command.command || "NONE";
  if (dom.activeProgress) {
    const progress = snapshot.active_command.progress_pct || 0;
    dom.activeProgress.textContent = `${Math.round(progress)}%`;
  }
  if (dom.stateName) dom.stateName.textContent = snapshot.state_name;
  if (dom.serverUptime) {
    const uptime = snapshot.diagnostics.server_uptime_sec || 0;
    dom.serverUptime.textContent = `${Math.floor(uptime)}s`;
  }
  if (dom.lastReject) dom.lastReject.textContent = snapshot.diagnostics.last_rejected_command || "-";
  if (dom.lastError) dom.lastError.textContent = snapshot.diagnostics.last_error || "-";

  // MTP mode information
  if (dom.modeCommandEn) dom.modeCommandEn.textContent = snapshot.mtp_modes.command_en ? "EN" : "OFF";
  if (dom.modeCommandEnReason) dom.modeCommandEnReason.textContent = snapshot.mtp_modes.command_en_reason || "";
  if (dom.modeApplyEn) dom.modeApplyEn.textContent = snapshot.mtp_modes.apply_en ? "EN" : "OFF";
  if (dom.modeApplyEnReason) dom.modeApplyEnReason.textContent = snapshot.mtp_modes.apply_en_reason || "";
  if (dom.blockingCondition) dom.blockingCondition.textContent = snapshot.mtp_state_machine.blocking_condition || "";

  // Update Airlock HMI
  if (dom.gPressure) dom.gPressure.value = snapshot.pressure_pa;
  if (dom.vPressure) dom.vPressure.textContent = snapshot.pressure_pa.toFixed(0);
  if (dom.gO2) dom.gO2.value = snapshot.o2_percent;
  if (dom.vO2) dom.vO2.textContent = snapshot.o2_percent.toFixed(2);

  if (dom.equalizeValve) dom.equalizeValve.style.fill = snapshot.equalize_valve_pct > 5 ? "#28a745" : "#6c757d";
  if (dom.ventValve) dom.ventValve.style.fill = snapshot.vent_valve_pct > 5 ? "#28a745" : "#6c757d";
  if (dom.pump) dom.pump.style.fill = snapshot.pump_on ? "#28a745" : "#343a40";

  updateAirlockCommandPanel(snapshot);

  // Update ECLSS HMI
  if (dom.eclssGPressure) dom.eclssGPressure.value = (snapshot.eclss_pressure_pa || 101325) / 1000;
  if (dom.eclssGCO2) dom.eclssGCO2.value = snapshot.eclss_co2_ppm || 0;
  if (dom.eclssVPressure) dom.eclssVPressure.textContent = ((snapshot.eclss_pressure_pa || 101325) / 1000).toFixed(1);
  if (dom.eclssVCO2) dom.eclssVCO2.textContent = (snapshot.eclss_co2_ppm || 0).toFixed(0);
  if (dom.eclssVWaterRecov) dom.eclssVWaterRecov.textContent = `${(snapshot.eclss_water_recovery_pct || 0).toFixed(1)}%`;
  if (dom.eclssVO2Gen) dom.eclssVO2Gen.textContent = `${(snapshot.eclss_o2_generation_kgph || 0).toFixed(2)} kg/h`;
  if (dom.eclssVCO2Cap) dom.eclssVCO2Cap.textContent = `${(snapshot.eclss_co2_capture_kgph || 0).toFixed(2)} kg/h`;
  if (dom.eclssVHumid) dom.eclssVHumid.textContent = `${(snapshot.eclss_humidity_pct || 0).toFixed(1)}%`;

  // Update Sabatier HMI
  if (dom.sabatierGTemp) dom.sabatierGTemp.value = snapshot.sabatier_reactor_temp_c || 0;
  if (dom.sabatierGCH4Feed) dom.sabatierGCH4Feed.value = snapshot.sabatier_co2_feed_kgph || 0;
  if (dom.sabatierVTemp) dom.sabatierVTemp.textContent = `${(snapshot.sabatier_reactor_temp_c || 0).toFixed(1)}`;
  if (dom.sabatierVCo2Feed) dom.sabatierVCo2Feed.textContent = `${(snapshot.sabatier_co2_feed_kgph || 0).toFixed(2)}`;
  if (dom.sabatierVPressure) dom.sabatierVPressure.textContent = `${(snapshot.sabatier_reactor_pressure_bar || 0).toFixed(2)} bar`;
  if (dom.sabatierVH2Feed) dom.sabatierVH2Feed.textContent = `${(snapshot.sabatier_h2_feed_kgph || 0).toFixed(2)} kg/h`;
  if (dom.sabatierVEfficiency) dom.sabatierVEfficiency.textContent = `${(snapshot.sabatier_conversion_efficiency_pct || 0).toFixed(1)}%`;
  if (dom.sabatierVMethane) dom.sabatierVMethane.textContent = `${(snapshot.sabatier_methane_production_kgph || 0).toFixed(2)} kg/h`;

  updateSubsystemCommandPanels(snapshot);

  renderEvents(snapshot);
  renderSessions(snapshot);

  // flash any new events in the 3D scene
  const lastTs = state.lastEventTimestamp || 0;
  let maxTs = lastTs;
  for (const e of snapshot.event_log) {
    if (e.timestamp_ms > lastTs) {
      if (state.mainScene && typeof state.mainScene.flashMessage === 'function') {
        state.mainScene.flashMessage(e.message);
      }
    }
    if (e.timestamp_ms > maxTs) maxTs = e.timestamp_ms;
  }
  state.lastEventTimestamp = maxTs;
}

function renderEvents(snapshot) {
  dom.eventLog.innerHTML = "";
  const events = [...snapshot.event_log].slice(-10).reverse();
  for (const entry of events) {
    const row = document.createElement("div");
    row.className = `event-row ${entry.severity.toLowerCase()}`;
    row.innerHTML = `<div class="meta">${new Date(entry.timestamp_ms).toLocaleTimeString()}</div><div>${entry.message}</div>`;
    dom.eventLog.appendChild(row);
  }
}

function renderSessions(snapshot) {
  dom.sessionList.innerHTML = "";
  snapshot.active_sessions.forEach(s => {
    const div = document.createElement("div");
    div.className = "session-row";
    div.textContent = `${s.client_id} | ${s.remote_addr} | ${s.authorized ? 'AUTH' : 'ANON'}`;
    dom.sessionList.appendChild(div);
  });
}

async function showAlert(msg) {
  // simple temporary banner
  let alert = document.getElementById("alert-banner");
  if (!alert) {
    alert = document.createElement("div");
    alert.id = "alert-banner";
    alert.style.position = "fixed";
    alert.style.top = "0";
    alert.style.left = "0";
    alert.style.width = "100%";
    alert.style.background = "rgba(255,100,100,0.9)";
    alert.style.color = "white";
    alert.style.padding = "0.5rem";
    alert.style.zIndex = "10000";
    alert.style.textAlign = "center";
    document.body.appendChild(alert);
  }
  alert.textContent = msg;
  setTimeout(() => { if (alert.parentNode) alert.parentNode.removeChild(alert); }, 4000);
}

async function sendOperatorCommand(command, p1, p2) {
  try {
    const payload = {
      sequence_id: state.sequenceId++,
      command,
      param1: p1,
      param2: p2,
      execute: true,
    };
    const res = await fetch("/api/commands/operator/write", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      const text = await res.text();
      await showAlert(`Command request failed: ${text || res.status}`);
      return;
    }

    const body = await res.json();
    if (body.status === "REJECTED") {
      await showAlert(body.reject_reason || "Command rejected");
    }
    await fetchSnapshot();
  } catch (error) {
    await showAlert(`Command transport error: ${error instanceof Error ? error.message : "Unknown error"}`);
  }
}

async function sendSubsystemLifecycle(system, action) {
  try {
    const subsystem = SUBSYSTEMS[system];
    if (!subsystem) {
      await showAlert(`Unknown subsystem: ${system}`);
      return;
    }

    const res = await fetch(`/api/v1/pea/${subsystem.peaId}/${action}`, {
      method: "POST",
    });
    if (!res.ok) {
      const text = await res.text();
      await showAlert(`Subsystem ${action} failed: ${text || res.status}`);
      return;
    }
    await fetchSnapshot();
    await refreshSubsystemOperatorState(system);
  } catch (error) {
    await showAlert(`Subsystem transport error: ${error instanceof Error ? error.message : "Unknown error"}`);
  }
}

async function setDoorTarget(door, pct) {
  const command = door === "inner" ?
    "SET_INNER_DOOR_TARGET_PCT" :
    "SET_OUTER_DOOR_TARGET_PCT";
  await sendOperatorCommand(command, pct, 0);
}

function evaluateAirlockCommand(snapshot, target) {
  if (!snapshot) {
    return { allowed: false, reason: "No process snapshot available" };
  }
  if (!snapshot.permissions?.operator_control_enabled) {
    return { allowed: false, reason: "Permissions: operator control disabled" };
  }
  if (!snapshot.mtp_modes?.command_en && target.command !== "ABORT_CYCLE") {
    const reason = snapshot.mtp_modes?.command_en_reason || "CommandEn disabled";
    return { allowed: false, reason: `Blocked: ${reason}` };
  }
  if (snapshot.active_command?.state === "RUNNING" && target.command !== "ABORT_CYCLE") {
    return { allowed: false, reason: "Busy: command already active" };
  }

  const innerOpen = (snapshot.inner_door_position_pct || 0) > 0.1;
  const outerOpen = (snapshot.outer_door_position_pct || 0) > 0.1;

  switch (target.command) {
    case "UNLOCK_OUTER_DOOR":
      if ((snapshot.pressure_pa || 0) > (snapshot.outer_unlock_max_pressure_pa || 0)) {
        return { allowed: false, reason: "Blocked: pressure too high for outer unlock" };
      }
      if (innerOpen) return { allowed: false, reason: "Interlock: inner door open" };
      break;
    case "UNLOCK_INNER_DOOR":
      if ((snapshot.pressure_pa || 0) < (snapshot.inner_unlock_min_pressure_pa || 0)) {
        return { allowed: false, reason: "Blocked: pressure too low for inner unlock" };
      }
      if (outerOpen) return { allowed: false, reason: "Interlock: outer door open" };
      break;
    case "LOCK_OUTER_DOOR":
      if (outerOpen) return { allowed: false, reason: "Blocked: outer door must be closed before locking" };
      break;
    case "LOCK_INNER_DOOR":
      if (innerOpen) return { allowed: false, reason: "Blocked: inner door must be closed before locking" };
      break;
    case "SET_OUTER_DOOR_TARGET_PCT":
      if (target.param1 > 0 && innerOpen) return { allowed: false, reason: "Interlock: inner door open" };
      if (target.param1 > 0 && snapshot.outer_lock_engaged) return { allowed: false, reason: "Blocked: outer door locked" };
      break;
    case "SET_INNER_DOOR_TARGET_PCT":
      if (target.param1 > 0 && outerOpen) return { allowed: false, reason: "Interlock: outer door open" };
      if (target.param1 > 0 && snapshot.inner_lock_engaged) return { allowed: false, reason: "Blocked: inner door locked" };
      break;
    case "START_DEPRESSURIZE_CYCLE":
    case "START_PRESSURIZE_CYCLE":
      if (innerOpen) return { allowed: false, reason: "Interlock: inner door open" };
      if (outerOpen) return { allowed: false, reason: "Interlock: outer door open" };
      break;
    default:
      break;
  }

  return { allowed: true, reason: "" };
}

function updateAirlockCommandPanel(snapshot) {
  const activeCommand = snapshot.active_command?.command || "NONE";
  const activeState = snapshot.active_command?.state || "IDLE";
  const rejected = snapshot.diagnostics?.last_rejected_command || "";
  const machineBlocking = snapshot.mtp_state_machine?.blocking_condition || "";
  const activeBlocking = snapshot.active_command?.blocking_condition || "";
  const blockingPattern = /^(blocked|interlock|busy|permissions)/i;
  const blockReason = rejected
    || (blockingPattern.test(machineBlocking) ? machineBlocking : "")
    || (blockingPattern.test(activeBlocking) ? activeBlocking : "")
    || "None";

  if (dom.cmdCurrentState) {
    dom.cmdCurrentState.textContent = `${snapshot.state_name || "unknown"} (${activeState})`;
  }
  if (dom.cmdCurrentActive) {
    dom.cmdCurrentActive.textContent = activeCommand;
  }
  if (dom.cmdBlockedReason) {
    dom.cmdBlockedReason.textContent = blockReason;
  }
  if (dom.airlockStatusMsg) {
    dom.airlockStatusMsg.textContent = blockReason === "None"
      ? "Ready"
      : `Attention: ${blockReason}`;
  }

  const allowedLabels = [];
  AIRLOCK_UI_COMMANDS.forEach((target) => {
    const button = document.querySelector(target.selector);
    if (!button) return;

    const evalResult = evaluateAirlockCommand(snapshot, target);
    button.disabled = !evalResult.allowed;
    button.title = evalResult.allowed ? `${target.label} allowed` : evalResult.reason;
    button.setAttribute("aria-disabled", evalResult.allowed ? "false" : "true");

    if (evalResult.allowed) {
      allowedLabels.push(target.label);
    }
  });

  if (dom.cmdAllowedList) {
    dom.cmdAllowedList.innerHTML = "";
    if (!allowedLabels.length) {
      const li = document.createElement("li");
      li.textContent = "No operator commands are currently permitted";
      dom.cmdAllowedList.appendChild(li);
    } else {
      allowedLabels.forEach((label) => {
        const li = document.createElement("li");
        li.textContent = label;
        dom.cmdAllowedList.appendChild(li);
      });
    }
  }
}

async function refreshSubsystemOperatorState(system) {
  const subsystem = SUBSYSTEMS[system];
  if (!subsystem) return;
  try {
    const res = await fetch(`/api/v1/pea/${subsystem.peaId}/operator-state`);
    if (!res.ok) return;
    const payload = await res.json();
    state.subsystemViews[system].operator = payload;
  } catch (_) {
    // best-effort refresh
  }
}

async function sendSubsystemAdvancedCommand(system, command, param1 = 0, param2 = 0) {
  const subsystem = SUBSYSTEMS[system];
  if (!subsystem) {
    await showAlert(`Unknown subsystem: ${system}`);
    return;
  }

  try {
    const res = await fetch(`/api/v1/pea/${subsystem.peaId}/advanced-command`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        source: "operator",
        sequence_id: state.sequenceId++,
        command,
        param1,
        param2,
      }),
    });
    if (!res.ok) {
      const text = await res.text();
      await showAlert(`Subsystem command failed: ${text || res.status}`);
      return;
    }
    const body = await res.json();
    if (body.status === "REJECTED") {
      await showAlert(body.reject_reason || "Subsystem command rejected");
    }
    if (body.operator_state) {
      state.subsystemViews[system].operator = {
        ...(state.subsystemViews[system].operator || {}),
        operator_state: body.operator_state,
        runtime: body.runtime,
        derived_service_state: body.runtime?.running ? "Execute" : "Idle",
      };
    }
    await fetchSnapshot();
    await refreshSubsystemOperatorState(system);
  } catch (error) {
    await showAlert(`Subsystem command transport error: ${error instanceof Error ? error.message : "Unknown error"}`);
  }
}

function evaluateSubsystemCommand(system, snapshot, def) {
  const operatorWrap = state.subsystemViews[system].operator;
  if (!operatorWrap) {
    return { allowed: false, reason: "No operator-state context" };
  }

  const runtime = operatorWrap.runtime || {};
  const operator = operatorWrap.operator_state || {};

  if (!runtime.deployed) return { allowed: false, reason: "Blocked: PEA not deployed" };
  if (!runtime.running) return { allowed: false, reason: "Blocked: PEA not running" };
  if (["OFF", "MAINT"].includes(operator.operation_mode)) {
    return { allowed: false, reason: "Blocked: subsystem operation mode is Off/Maint" };
  }
  if (!operator.command_en) {
    return { allowed: false, reason: operator.command_en_reason || "Blocked: CommandEn disabled" };
  }
  if (!operator.operator_control_enabled) {
    return { allowed: false, reason: "Blocked: operator control disabled" };
  }

  if (system === "eclss") {
    const scrubber = !!snapshot.eclss_co2_scrubber_running;
    const electrolyzer = !!snapshot.eclss_electrolyzer_running;
    const water = !!snapshot.eclss_water_processor_running;
    const isolated = !!snapshot.eclss_branch_isolated;
    const safeHaven = !!snapshot.eclss_safe_haven_enabled;

    switch (def.command) {
      case "START_CO2_SCRUBBER_BED_A":
        if (scrubber) return { allowed: false, reason: "Already running" };
        break;
      case "STOP_CO2_SCRUBBER_BED_A":
        if (!scrubber) return { allowed: false, reason: "Already stopped" };
        break;
      case "START_ELECTROLYZER_STACK":
        if (electrolyzer) return { allowed: false, reason: "Already running" };
        break;
      case "STOP_ELECTROLYZER_STACK":
        if (!electrolyzer) return { allowed: false, reason: "Already stopped" };
        break;
      case "START_WATER_PROCESSOR":
        if (water) return { allowed: false, reason: "Already running" };
        break;
      case "STOP_WATER_PROCESSOR":
        if (!water) return { allowed: false, reason: "Already stopped" };
        break;
      case "ISOLATE_ECLSS_BRANCH":
        if (isolated) return { allowed: false, reason: "Branch already isolated" };
        break;
      case "RESTORE_ECLSS_BRANCH":
        if (!isolated) return { allowed: false, reason: "Branch already restored" };
        break;
      case "ENABLE_SAFE_HAVEN":
        if (safeHaven) return { allowed: false, reason: "Safe-haven already enabled" };
        break;
      case "DISABLE_SAFE_HAVEN":
        if (!safeHaven) return { allowed: false, reason: "Safe-haven already disabled" };
        break;
      default:
        break;
    }
  } else {
    const meth = !!snapshot.sabatier_methanation_enabled;
    const feedCond = !!snapshot.sabatier_feed_conditioning_enabled;
    const h2Recovery = !!snapshot.sabatier_hydrogen_recovery_enabled;
    const regen = !!snapshot.sabatier_catalyst_regen_active;
    const vent = !!snapshot.sabatier_emergency_vent_active;

    switch (def.command) {
      case "START_METHANATION":
        if (meth) return { allowed: false, reason: "Already running" };
        break;
      case "STOP_METHANATION":
        if (!meth) return { allowed: false, reason: "Already stopped" };
        break;
      case "START_FEED_CONDITIONING":
        if (feedCond) return { allowed: false, reason: "Already running" };
        break;
      case "STOP_FEED_CONDITIONING":
        if (!feedCond) return { allowed: false, reason: "Already stopped" };
        break;
      case "START_HYDROGEN_RECOVERY":
        if (h2Recovery) return { allowed: false, reason: "Already running" };
        break;
      case "STOP_HYDROGEN_RECOVERY":
        if (!h2Recovery) return { allowed: false, reason: "Already stopped" };
        break;
      case "START_CATALYST_REGEN":
        if (regen) return { allowed: false, reason: "Already running" };
        break;
      case "STOP_CATALYST_REGEN":
        if (!regen) return { allowed: false, reason: "Already stopped" };
        break;
      case "EXECUTE_EMERGENCY_VENT":
        if (vent) return { allowed: false, reason: "Emergency vent already active" };
        break;
      default:
        break;
    }
  }

  return { allowed: true, reason: "" };
}

function updateSubsystemCommandPanels(snapshot) {
  const panelDefs = [
    {
      system: "eclss",
      commands: ECLSS_UI_COMMANDS,
      currentStateEl: dom.eclssCmdCurrentState,
      activeProcEl: dom.eclssCmdActiveProcedure,
      lastCmdEl: dom.eclssCmdLastCommand,
      lastStatusEl: dom.eclssCmdLastStatus,
      lastReasonEl: dom.eclssCmdLastReason,
      allowedListEl: dom.eclssCmdAllowedList,
    },
    {
      system: "sabatier",
      commands: SABATIER_UI_COMMANDS,
      currentStateEl: dom.sabatierCmdCurrentState,
      activeProcEl: dom.sabatierCmdActiveProcedure,
      lastCmdEl: dom.sabatierCmdLastCommand,
      lastStatusEl: dom.sabatierCmdLastStatus,
      lastReasonEl: dom.sabatierCmdLastReason,
      allowedListEl: dom.sabatierCmdAllowedList,
    },
  ];

  panelDefs.forEach((panel) => {
    const operatorWrap = state.subsystemViews[panel.system].operator || {};
    const operator = operatorWrap.operator_state || {};
    const derived = operatorWrap.derived_service_state || "unknown";

    if (panel.currentStateEl) panel.currentStateEl.textContent = derived;
    if (panel.activeProcEl) panel.activeProcEl.textContent = operator.active_procedure || "None";
    if (panel.lastCmdEl) panel.lastCmdEl.textContent = operator.last_command || "None";
    if (panel.lastStatusEl) panel.lastStatusEl.textContent = operator.last_command_status || "IDLE";
    if (panel.lastReasonEl) panel.lastReasonEl.textContent = operator.last_reject_reason || "None";

    const allowed = [];
    panel.commands.forEach((def) => {
      const button = document.querySelector(def.selector);
      if (!button) return;
      const evalResult = evaluateSubsystemCommand(panel.system, snapshot, def);
      button.disabled = !evalResult.allowed;
      button.title = evalResult.allowed ? `${def.label} allowed` : evalResult.reason;
      if (evalResult.allowed) allowed.push(def.label);
    });

    if (panel.allowedListEl) {
      panel.allowedListEl.innerHTML = "";
      if (!allowed.length) {
        const li = document.createElement("li");
        li.textContent = "No subsystem commands are currently permitted";
        panel.allowedListEl.appendChild(li);
      } else {
        allowed.forEach((label) => {
          const li = document.createElement("li");
          li.textContent = label;
          panel.allowedListEl.appendChild(li);
        });
      }
    }
  });
}

async function initThree() {
  try {
    const { UnderhillBaseScene } = await import("./scene.js");
    state.mainScene = new UnderhillBaseScene(dom.threeWrap);
    let debugEnabled = false;
    try {
      debugEnabled = window.localStorage.getItem(LAYOUT_DEBUG_STORAGE_KEY) === "1";
    } catch (_) {
      debugEnabled = false;
    }
    if (dom.layoutDebugToggle) {
      dom.layoutDebugToggle.checked = debugEnabled;
    }
    state.mainScene.setLayoutDebug(debugEnabled);
  } catch (e) {
    console.error("Three.js initialization failed:", e);
  }
}

bootstrap();
