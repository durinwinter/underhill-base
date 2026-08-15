use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const MAINTENANCE_SCHEMA_VERSION: u32 = 1;
const MAX_RETAINED_COMPLETED_WORK_ORDERS: usize = 2_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkOrderStatus {
    Queued,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceWorkOrder {
    pub work_order_id: String,
    pub target_pea_id: String,
    pub target_asset_id: String,
    pub work_type: String,
    pub priority: u8,
    pub status: WorkOrderStatus,
    pub created_plant_sec: f64,
    pub started_plant_sec: Option<f64>,
    pub completed_plant_sec: Option<f64>,
    pub required_labor_hours: f64,
    pub remaining_labor_hours: f64,
    pub crew_technicians_required: f64,
    pub robots_required: f64,
    pub tool_id: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolState {
    pub tool_id: String,
    pub total_quantity: u32,
    pub reserved_quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEvent {
    pub event_kind: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct MaintenanceAllocation {
    pub work_order_id: String,
    pub target_pea_id: String,
    pub target_asset_id: String,
    pub labor_hours: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceSnapshot {
    pub schema_version: u32,
    pub timestamp_ms: u64,
    pub sim_time_sec: f64,
    pub crew_technicians_total: f64,
    pub crew_technicians_available: f64,
    pub robots_total: f64,
    pub robots_available: f64,
    pub queued_work_orders: usize,
    pub active_work_orders: usize,
    pub blocked_work_orders: usize,
    pub retained_completed_work_orders: usize,
    pub total_completed_work_orders: u64,
    pub total_cancelled_work_orders: u64,
    pub warehouse_spares: BTreeMap<String, u32>,
    pub tools: Vec<ToolState>,
    pub work_orders: Vec<MaintenanceWorkOrder>,
    pub power_kw: f64,
    pub service_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSimulation {
    schema_version: u32,
    sim_time_sec: f64,
    crew_technicians_total: f64,
    robots_total: f64,
    tools: BTreeMap<String, ToolState>,
    warehouse_spares: BTreeMap<String, u32>,
    work_orders: Vec<MaintenanceWorkOrder>,
    total_completed_work_orders: u64,
    total_cancelled_work_orders: u64,
    power_kw: f64,
    service_available: bool,
    #[serde(default)]
    pending_events: Vec<MaintenanceEvent>,
}

impl Default for MaintenanceSimulation {
    fn default() -> Self {
        Self::new()
    }
}

impl MaintenanceSimulation {
    pub fn new() -> Self {
        let tools = [
            ("oru_service_cart", 2),
            ("gas_loop_isolation_kit", 1),
            ("water_loop_service_kit", 1),
            ("electrical_diagnostics_kit", 2),
        ]
        .into_iter()
        .map(|(tool_id, total_quantity)| {
            (
                tool_id.to_string(),
                ToolState {
                    tool_id: tool_id.to_string(),
                    total_quantity,
                    reserved_quantity: 0,
                },
            )
        })
        .collect();
        let warehouse_spares = [
            ("oga_water_assembly_oru", 4),
            ("cdra_desiccant_adsorbent_assembly", 4),
            ("ccaa_water_separator_oru", 4),
            ("wpa_pump_separator", 4),
        ]
        .into_iter()
        .map(|(sku, quantity)| (sku.to_string(), quantity))
        .collect();
        Self {
            schema_version: MAINTENANCE_SCHEMA_VERSION,
            sim_time_sec: 0.0,
            crew_technicians_total: 1.5,
            robots_total: 2.0,
            tools,
            warehouse_spares,
            work_orders: Vec::new(),
            total_completed_work_orders: 0,
            total_cancelled_work_orders: 0,
            power_kw: 0.6,
            service_available: true,
            pending_events: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn enqueue_work_order(
        &mut self,
        work_order_id: String,
        target_pea_id: String,
        target_asset_id: String,
        work_type: String,
        priority: u8,
        required_labor_hours: f64,
        crew_technicians_required: f64,
        robots_required: f64,
        tool_id: String,
    ) -> Result<(), String> {
        if work_order_id.trim().is_empty()
            || target_pea_id.trim().is_empty()
            || target_asset_id.trim().is_empty()
            || work_type.trim().is_empty()
        {
            return Err("work-order identity, target, and type are required".to_string());
        }
        if self
            .work_orders
            .iter()
            .any(|work_order| work_order.work_order_id == work_order_id)
        {
            return Err(format!("duplicate work order: {work_order_id}"));
        }
        if priority > 100 {
            return Err("priority must be within 0..=100".to_string());
        }
        for (name, value) in [
            ("required_labor_hours", required_labor_hours),
            ("crew_technicians_required", crew_technicians_required),
            ("robots_required", robots_required),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(format!("{name} must be finite and positive"));
            }
        }
        if !self.tools.contains_key(&tool_id) {
            return Err(format!("unknown maintenance tool: {tool_id}"));
        }
        self.work_orders.push(MaintenanceWorkOrder {
            work_order_id: work_order_id.clone(),
            target_pea_id,
            target_asset_id,
            work_type,
            priority,
            status: WorkOrderStatus::Queued,
            created_plant_sec: self.sim_time_sec,
            started_plant_sec: None,
            completed_plant_sec: None,
            required_labor_hours,
            remaining_labor_hours: required_labor_hours,
            crew_technicians_required,
            robots_required,
            tool_id,
            blocked_reason: None,
        });
        self.pending_events.push(MaintenanceEvent {
            event_kind: "work_order_queued".to_string(),
            subject: work_order_id,
            detail: "maintenance demand entered the shared priority queue".to_string(),
        });
        Ok(())
    }

    pub fn step(
        &mut self,
        dt_sec: f64,
        running: bool,
        critical_power_available: bool,
    ) -> Vec<MaintenanceAllocation> {
        if dt_sec <= 0.0 || !dt_sec.is_finite() {
            return Vec::new();
        }
        self.sim_time_sec += dt_sec;
        self.service_available = running && critical_power_available;
        self.start_eligible_work_orders();
        if !self.service_available {
            self.power_kw = 0.15;
            return Vec::new();
        }

        let mut active_indices: Vec<_> = self
            .work_orders
            .iter()
            .enumerate()
            .filter(|(_, work_order)| work_order.status == WorkOrderStatus::InProgress)
            .map(|(index, _)| index)
            .collect();
        active_indices.sort_by(|left, right| {
            self.work_orders[*right]
                .priority
                .cmp(&self.work_orders[*left].priority)
                .then_with(|| {
                    self.work_orders[*left]
                        .created_plant_sec
                        .total_cmp(&self.work_orders[*right].created_plant_sec)
                })
        });
        let mut crew_available = self.crew_technicians_total;
        let mut robots_available = self.robots_total;
        let mut robot_utilization = 0.0;
        let dt_hours = dt_sec / 3_600.0;
        let mut allocations = Vec::new();
        let mut completed_indices = Vec::new();
        for index in active_indices {
            let work_order = &mut self.work_orders[index];
            let rate = (crew_available / work_order.crew_technicians_required)
                .min(robots_available / work_order.robots_required)
                .clamp(0.0, 1.0);
            if rate <= 0.0 {
                work_order.blocked_reason =
                    Some("shared crew or robot capacity exhausted".to_string());
                continue;
            }
            work_order.blocked_reason = None;
            let labor_hours = (dt_hours * rate).min(work_order.remaining_labor_hours);
            work_order.remaining_labor_hours =
                (work_order.remaining_labor_hours - labor_hours).max(0.0);
            crew_available -= work_order.crew_technicians_required * rate;
            robots_available -= work_order.robots_required * rate;
            robot_utilization += work_order.robots_required * rate;
            allocations.push(MaintenanceAllocation {
                work_order_id: work_order.work_order_id.clone(),
                target_pea_id: work_order.target_pea_id.clone(),
                target_asset_id: work_order.target_asset_id.clone(),
                labor_hours,
            });
            if work_order.remaining_labor_hours <= f64::EPSILON {
                completed_indices.push(index);
            }
        }
        for index in completed_indices {
            let work_order_id = self.work_orders[index].work_order_id.clone();
            self.release_tool_for_index(index);
            self.work_orders[index].status = WorkOrderStatus::Completed;
            self.work_orders[index].completed_plant_sec = Some(self.sim_time_sec);
            self.total_completed_work_orders += 1;
            self.pending_events.push(MaintenanceEvent {
                event_kind: "work_order_completed".to_string(),
                subject: work_order_id,
                detail: "required labor was delivered by shared maintenance resources".to_string(),
            });
        }
        self.power_kw = 0.6 + robot_utilization * 0.9;
        self.prune_completed_history();
        allocations
    }

    pub fn cancel_work_order(&mut self, work_order_id: &str) -> Result<bool, String> {
        let index = self
            .work_orders
            .iter()
            .position(|work_order| work_order.work_order_id == work_order_id)
            .ok_or_else(|| format!("unknown work order: {work_order_id}"))?;
        if matches!(
            self.work_orders[index].status,
            WorkOrderStatus::Completed | WorkOrderStatus::Cancelled
        ) {
            return Ok(false);
        }
        if self.work_orders[index].status == WorkOrderStatus::InProgress {
            self.release_tool_for_index(index);
        }
        self.work_orders[index].status = WorkOrderStatus::Cancelled;
        self.work_orders[index].blocked_reason = None;
        self.total_cancelled_work_orders += 1;
        self.pending_events.push(MaintenanceEvent {
            event_kind: "work_order_cancelled".to_string(),
            subject: work_order_id.to_string(),
            detail: "bounded validation demand was removed from the live queue".to_string(),
        });
        Ok(true)
    }

    pub fn receive_spares(&mut self, sku: &str, quantity: u32) -> Result<(), String> {
        validate_inventory_mutation(sku, quantity)?;
        let entry = self.warehouse_spares.entry(sku.to_string()).or_default();
        *entry = entry
            .checked_add(quantity)
            .filter(|total| *total <= 100_000)
            .ok_or_else(|| "warehouse SKU inventory cannot exceed 100000".to_string())?;
        self.pending_events.push(MaintenanceEvent {
            event_kind: "warehouse_spares_received".to_string(),
            subject: sku.to_string(),
            detail: format!("{quantity} unit(s) crossed the external logistics boundary"),
        });
        Ok(())
    }

    pub fn dispatch_spares(&mut self, sku: &str, quantity: u32) -> Result<(), String> {
        validate_inventory_mutation(sku, quantity)?;
        let entry = self
            .warehouse_spares
            .get_mut(sku)
            .ok_or_else(|| format!("unknown warehouse SKU: {sku}"))?;
        if *entry < quantity {
            return Err(format!(
                "insufficient warehouse stock for {sku}: requested {quantity}, available {entry}"
            ));
        }
        *entry -= quantity;
        self.pending_events.push(MaintenanceEvent {
            event_kind: "warehouse_spares_dispatched".to_string(),
            subject: sku.to_string(),
            detail: format!("{quantity} unit(s) dispatched to point-of-use inventory"),
        });
        Ok(())
    }

    pub fn snapshot(&self) -> MaintenanceSnapshot {
        let queued_work_orders = self
            .work_orders
            .iter()
            .filter(|work_order| work_order.status == WorkOrderStatus::Queued)
            .count();
        let active_work_orders = self
            .work_orders
            .iter()
            .filter(|work_order| work_order.status == WorkOrderStatus::InProgress)
            .count();
        let blocked_work_orders = self
            .work_orders
            .iter()
            .filter(|work_order| work_order.blocked_reason.is_some())
            .count();
        let retained_completed_work_orders = self
            .work_orders
            .iter()
            .filter(|work_order| work_order.status == WorkOrderStatus::Completed)
            .count();
        let allocated_crew: f64 = self
            .work_orders
            .iter()
            .filter(|work_order| work_order.status == WorkOrderStatus::InProgress)
            .map(|work_order| work_order.crew_technicians_required)
            .sum::<f64>()
            .min(self.crew_technicians_total);
        let allocated_robots: f64 = self
            .work_orders
            .iter()
            .filter(|work_order| work_order.status == WorkOrderStatus::InProgress)
            .map(|work_order| work_order.robots_required)
            .sum::<f64>()
            .min(self.robots_total);
        MaintenanceSnapshot {
            schema_version: self.schema_version,
            timestamp_ms: crate::sim::Simulation::now_ms(),
            sim_time_sec: self.sim_time_sec,
            crew_technicians_total: self.crew_technicians_total,
            crew_technicians_available: (self.crew_technicians_total - allocated_crew).max(0.0),
            robots_total: self.robots_total,
            robots_available: (self.robots_total - allocated_robots).max(0.0),
            queued_work_orders,
            active_work_orders,
            blocked_work_orders,
            retained_completed_work_orders,
            total_completed_work_orders: self.total_completed_work_orders,
            total_cancelled_work_orders: self.total_cancelled_work_orders,
            warehouse_spares: self.warehouse_spares.clone(),
            tools: self.tools.values().cloned().collect(),
            work_orders: self.work_orders.clone(),
            power_kw: self.power_kw,
            service_available: self.service_available,
        }
    }

    pub fn drain_events(&mut self) -> Vec<MaintenanceEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn mtp_nodes(&self) -> Vec<String> {
        vec![
            "ServiceSet/MaintenanceService/ServiceInformation".to_string(),
            "ServiceSet/MaintenanceService/Modes".to_string(),
            "ServiceSet/MaintenanceService/StateMachine".to_string(),
            "ServiceSet/MaintenanceService/DataAssemblies/WorkQueue".to_string(),
            "ServiceSet/MaintenanceService/DataAssemblies/Resources".to_string(),
            "ServiceSet/MaintenanceService/DataAssemblies/Inventory".to_string(),
            "ServiceSet/MaintenanceService/DataAssemblies/Diagnostics".to_string(),
        ]
    }

    fn start_eligible_work_orders(&mut self) {
        if !self.service_available {
            return;
        }
        let mut queued_indices: Vec<_> = self
            .work_orders
            .iter()
            .enumerate()
            .filter(|(_, work_order)| work_order.status == WorkOrderStatus::Queued)
            .map(|(index, _)| index)
            .collect();
        queued_indices.sort_by(|left, right| {
            self.work_orders[*right]
                .priority
                .cmp(&self.work_orders[*left].priority)
                .then_with(|| {
                    self.work_orders[*left]
                        .created_plant_sec
                        .total_cmp(&self.work_orders[*right].created_plant_sec)
                })
        });
        for index in queued_indices {
            let tool_id = self.work_orders[index].tool_id.clone();
            let tool = self.tools.get_mut(&tool_id).expect("validated tool exists");
            if tool.reserved_quantity >= tool.total_quantity {
                self.work_orders[index].blocked_reason =
                    Some(format!("required tool unavailable: {tool_id}"));
                continue;
            }
            tool.reserved_quantity += 1;
            self.work_orders[index].status = WorkOrderStatus::InProgress;
            self.work_orders[index].started_plant_sec = Some(self.sim_time_sec);
            self.work_orders[index].blocked_reason = None;
            self.pending_events.push(MaintenanceEvent {
                event_kind: "work_order_started".to_string(),
                subject: self.work_orders[index].work_order_id.clone(),
                detail: format!("reserved tool {tool_id}"),
            });
        }
    }

    fn release_tool_for_index(&mut self, index: usize) {
        let tool_id = self.work_orders[index].tool_id.clone();
        if let Some(tool) = self.tools.get_mut(&tool_id) {
            tool.reserved_quantity = tool.reserved_quantity.saturating_sub(1);
        }
    }

    fn prune_completed_history(&mut self) {
        let completed_count = self
            .work_orders
            .iter()
            .filter(|work_order| work_order.status == WorkOrderStatus::Completed)
            .count();
        let mut remove_count = completed_count.saturating_sub(MAX_RETAINED_COMPLETED_WORK_ORDERS);
        if remove_count == 0 {
            return;
        }
        self.work_orders.retain(|work_order| {
            if remove_count > 0 && work_order.status == WorkOrderStatus::Completed {
                remove_count -= 1;
                false
            } else {
                true
            }
        });
    }
}

fn validate_inventory_mutation(sku: &str, quantity: u32) -> Result<(), String> {
    if sku.trim().is_empty() {
        return Err("SKU is required".to_string());
    }
    if !(1..=10_000).contains(&quantity) {
        return Err("quantity must be within 1..=10000".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enqueue_test_order(
        simulation: &mut MaintenanceSimulation,
        id: &str,
        priority: u8,
        hours: f64,
        tool: &str,
    ) {
        simulation
            .enqueue_work_order(
                id.to_string(),
                "ECLSS-PEA-001".to_string(),
                format!("asset-{id}"),
                "replace_oru".to_string(),
                priority,
                hours,
                1.0,
                1.0,
                tool.to_string(),
            )
            .unwrap();
    }

    #[test]
    fn work_does_not_progress_without_service_and_power() {
        let mut simulation = MaintenanceSimulation::new();
        enqueue_test_order(&mut simulation, "wo-1", 90, 2.0, "oru_service_cart");
        assert!(simulation.step(3_600.0, false, true).is_empty());
        assert!(simulation.step(3_600.0, true, false).is_empty());
        assert_eq!(
            simulation.snapshot().work_orders[0].remaining_labor_hours,
            2.0
        );
    }

    #[test]
    fn shared_capacity_prioritizes_and_fractionally_advances_work() {
        let mut simulation = MaintenanceSimulation::new();
        enqueue_test_order(&mut simulation, "low", 10, 2.0, "oru_service_cart");
        enqueue_test_order(&mut simulation, "high", 90, 2.0, "oru_service_cart");
        let allocations = simulation.step(3_600.0, true, true);
        assert_eq!(allocations.len(), 2);
        let high = allocations
            .iter()
            .find(|allocation| allocation.work_order_id == "high")
            .unwrap();
        let low = allocations
            .iter()
            .find(|allocation| allocation.work_order_id == "low")
            .unwrap();
        assert_eq!(high.labor_hours, 1.0);
        assert_eq!(low.labor_hours, 0.5);
    }

    #[test]
    fn scarce_tool_blocks_second_order_until_release() {
        let mut simulation = MaintenanceSimulation::new();
        enqueue_test_order(&mut simulation, "first", 90, 1.0, "gas_loop_isolation_kit");
        enqueue_test_order(&mut simulation, "second", 80, 1.0, "gas_loop_isolation_kit");
        simulation.step(3_600.0, true, true);
        let after_first = simulation.snapshot();
        assert_eq!(after_first.total_completed_work_orders, 1);
        assert_eq!(after_first.queued_work_orders, 1);
        simulation.step(3_600.0, true, true);
        assert_eq!(simulation.snapshot().total_completed_work_orders, 2);
    }

    #[test]
    fn inventory_is_finite_and_replenishable() {
        let mut simulation = MaintenanceSimulation::new();
        simulation
            .dispatch_spares("oga_water_assembly_oru", 4)
            .unwrap();
        assert!(
            simulation
                .dispatch_spares("oga_water_assembly_oru", 1)
                .is_err()
        );
        simulation
            .receive_spares("oga_water_assembly_oru", 3)
            .unwrap();
        assert_eq!(
            simulation.snapshot().warehouse_spares["oga_water_assembly_oru"],
            3
        );
    }

    #[test]
    fn cancelling_active_work_releases_its_reserved_tool() {
        let mut sim = MaintenanceSimulation::new();
        enqueue_test_order(&mut sim, "wo-cancel", 90, 1.0, "water_loop_service_kit");
        sim.step(1.0, true, true);
        assert_eq!(
            sim.snapshot()
                .tools
                .iter()
                .find(|tool| tool.tool_id == "water_loop_service_kit")
                .unwrap()
                .reserved_quantity,
            1
        );
        assert!(sim.cancel_work_order("wo-cancel").unwrap());
        assert!(!sim.cancel_work_order("wo-cancel").unwrap());
        assert_eq!(
            sim.snapshot()
                .tools
                .iter()
                .find(|tool| tool.tool_id == "water_loop_service_kit")
                .unwrap()
                .reserved_quantity,
            0
        );
    }

    #[test]
    fn active_queue_and_resources_survive_checkpoint_round_trip() {
        let mut simulation = MaintenanceSimulation::new();
        enqueue_test_order(&mut simulation, "wo-restore", 50, 3.0, "oru_service_cart");
        simulation.step(3_600.0, true, true);
        let encoded = serde_json::to_vec(&simulation).unwrap();
        let restored: MaintenanceSimulation = serde_json::from_slice(&encoded).unwrap();
        let snapshot = restored.snapshot();
        assert_eq!(snapshot.active_work_orders, 1);
        assert_eq!(snapshot.work_orders[0].remaining_labor_hours, 2.0);
        assert_eq!(
            snapshot
                .tools
                .iter()
                .find(|tool| tool.tool_id == "oru_service_cart")
                .unwrap()
                .reserved_quantity,
            1
        );
    }
}
