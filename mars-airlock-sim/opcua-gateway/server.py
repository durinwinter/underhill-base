#!/usr/bin/env python3
import asyncio
import json
import logging
import os
import time
from dataclasses import dataclass
from typing import Any, Dict

import aiohttp
from asyncua import Server, ua

logging.basicConfig(level=os.environ.get("LOG_LEVEL", "INFO").upper())
LOG = logging.getLogger("opcua-gateway")

BACKEND_BASE_URL = os.environ.get("BACKEND_BASE_URL", "http://mars-airlock:8080")
ENDPOINT = os.environ.get("OPCUA_ENDPOINT", "opc.tcp://0.0.0.0:4841/mars-airlock")
NAMESPACE_URI = os.environ.get("OPCUA_NAMESPACE", "urn:mars-airlock:mtp")
POLL_INTERVAL_SEC = float(os.environ.get("POLL_INTERVAL_SEC", "0.2"))

COMMAND_ID_TO_NAME = {
    0: "NONE",
    1: "START_DEPRESSURIZE_CYCLE",
    2: "START_PRESSURIZE_CYCLE",
    3: "ABORT_CYCLE",
    4: "RESET_FAULTS",
    5: "SET_PUMP_ON",
    6: "SET_EQUALIZE_VALVE_PCT",
    7: "SET_VENT_VALVE_PCT",
    8: "SET_INNER_DOOR_TARGET_PCT",
    9: "SET_OUTER_DOOR_TARGET_PCT",
    10: "LOCK_INNER_DOOR",
    11: "UNLOCK_INNER_DOOR",
    12: "LOCK_OUTER_DOOR",
    13: "UNLOCK_OUTER_DOOR",
}

STATUS_NAME_TO_ID = {
    "IDLE": 0,
    "ACCEPTED": 1,
    "REJECTED": 2,
    "RUNNING": 3,
    "COMPLETE": 4,
    "ABORTED": 5,
}


@dataclass
class RequestNodes:
    sequence_id: Any
    command: Any
    param1: Any
    param2: Any
    execute: Any


@dataclass
class ResponseNodes:
    ack_sequence_id: Any
    status: Any
    reject_reason: Any
    last_update_time_ms: Any


class OpcuaGateway:
    def __init__(self) -> None:
        self.server = Server()
        self.namespace_idx = 0
        self.http_session: aiohttp.ClientSession | None = None
        self.operator_req: RequestNodes | None = None
        self.operator_rsp: ResponseNodes | None = None
        self.remote_req: RequestNodes | None = None
        self.remote_rsp: ResponseNodes | None = None
        self.nodes: Dict[str, Any] = {}
        self.last_execute_state = {"operator": False, "remote": False}

    async def init(self) -> None:
        await self.server.init()
        self.server.set_endpoint(ENDPOINT)
        self.server.set_server_name("Mars Airlock OPC UA Gateway")
        self.namespace_idx = await self.server.register_namespace(NAMESPACE_URI)

        await self._build_address_space()

        self.http_session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=5),
            headers={"content-type": "application/json"},
        )

    async def _build_address_space(self) -> None:
        idx = self.namespace_idx
        objects = self.server.nodes.objects

        mars_base = await objects.add_object(idx, "MarsBase")
        airlock = await mars_base.add_object(idx, "AirlockPEA")

        pea_info = await airlock.add_object(idx, "PEAInformationLabel")
        self.nodes["tag_name"] = await pea_info.add_variable(idx, "TagName", "AIRLOCK-PEA-001")
        self.nodes["module_name"] = await pea_info.add_variable(idx, "ModuleName", "Underhill Airlock")
        self.nodes["manufacturer"] = await pea_info.add_variable(idx, "Manufacturer", "MarsSimWorks")
        self.nodes["software_revision"] = await pea_info.add_variable(idx, "SoftwareRevision", "0.1.0")
        self.nodes["mtp_version"] = await pea_info.add_variable(
            idx,
            "MTPVersion",
            "VDI/VDE/NAMUR 2658 (conceptual)",
        )
        self.nodes["health_state"] = await pea_info.add_variable(idx, "HealthState", "OK")

        diagnostics = await airlock.add_object(idx, "Diagnostics")
        self.nodes["endpoint_url"] = await diagnostics.add_variable(idx, "EndpointUrl", ENDPOINT)
        self.nodes["security_mode"] = await diagnostics.add_variable(idx, "SecurityMode", "NONE")
        self.nodes["connected_clients"] = await diagnostics.add_variable(idx, "ConnectedClientCount", 0)
        self.nodes["subscription_count"] = await diagnostics.add_variable(idx, "SubscriptionCount", 0)
        self.nodes["publish_rate"] = await diagnostics.add_variable(idx, "PublishingRateHz", 10.0)
        self.nodes["last_rejected"] = await diagnostics.add_variable(idx, "LastRejectedCommand", "")
        self.nodes["last_error"] = await diagnostics.add_variable(idx, "LastError", "")

        service_set = await airlock.add_object(idx, "ServiceSet")
        airlock_service = await service_set.add_object(idx, "AirlockService")

        service_info = await airlock_service.add_object(idx, "ServiceInformation")
        self.nodes["service_name"] = await service_info.add_variable(idx, "ServiceName", "AirlockService")
        self.nodes["active_procedure"] = await service_info.add_variable(idx, "ActiveProcedure", "None")

        modes = await airlock_service.add_object(idx, "Modes")
        self.nodes["operation_mode"] = await modes.add_variable(idx, "OperationMode", "AUTO")
        self.nodes["source_mode"] = await modes.add_variable(idx, "SourceMode", "SYSTEM_AUTO")
        self.nodes["command_en"] = await modes.add_variable(idx, "CommandEn", True)
        self.nodes["command_en_reason"] = await modes.add_variable(idx, "CommandEnReason", "")

        state_machine = await airlock_service.add_object(idx, "StateMachine")
        self.nodes["current_state"] = await state_machine.add_variable(idx, "CurrentState", "idle")
        self.nodes["time_in_state_sec"] = await state_machine.add_variable(idx, "TimeInStateSec", 0.0)
        self.nodes["transition_active"] = await state_machine.add_variable(idx, "TransitionActive", False)
        self.nodes["blocking_condition"] = await state_machine.add_variable(idx, "BlockingCondition", "")

        data_assemblies = await airlock_service.add_object(idx, "DataAssemblies")

        indicators = await data_assemblies.add_object(idx, "Indicators")
        self.nodes["pressure_pa"] = await indicators.add_variable(idx, "PressurePa", 101325.0)
        self.nodes["temperature_k"] = await indicators.add_variable(idx, "TemperatureK", 293.15)
        self.nodes["o2_percent"] = await indicators.add_variable(idx, "O2Percent", 21.0)
        self.nodes["inner_door_pct"] = await indicators.add_variable(idx, "InnerDoorPositionPct", 0.0)
        self.nodes["outer_door_pct"] = await indicators.add_variable(idx, "OuterDoorPositionPct", 0.0)
        self.nodes["pump_on"] = await indicators.add_variable(idx, "PumpOn", False)
        self.nodes["equalize_valve_pct"] = await indicators.add_variable(idx, "EqualizeValvePct", 0.0)
        self.nodes["vent_valve_pct"] = await indicators.add_variable(idx, "VentValvePct", 0.0)
        self.nodes["alarm_summary"] = await indicators.add_variable(idx, "AlarmSummary", "OK")

        parameters = await data_assemblies.add_object(idx, "Parameters")
        self.nodes["operator_control_enabled"] = await parameters.add_variable(
            idx,
            "OperatorControlEnabled",
            True,
        )
        self.nodes["remote_control_enabled"] = await parameters.add_variable(
            idx,
            "RemoteControlEnabled",
            True,
        )
        self.nodes["leak_rate_nominal"] = await parameters.add_variable(idx, "LeakRateNominal", 0.0005)

        control = await data_assemblies.add_object(idx, "Control")
        operator_obj = await control.add_object(idx, "OperatorCommands")
        remote_obj = await control.add_object(idx, "RemoteCommands")
        active_obj = await control.add_object(idx, "ActiveCommand")

        self.operator_req, self.operator_rsp = await self._add_command_branch(idx, operator_obj)
        self.remote_req, self.remote_rsp = await self._add_command_branch(idx, remote_obj)

        self.nodes["active_command"] = await active_obj.add_variable(idx, "Command", 0)
        self.nodes["active_source"] = await active_obj.add_variable(idx, "Source", "SYSTEM_AUTO")
        self.nodes["active_sequence"] = await active_obj.add_variable(idx, "SequenceId", 0)
        self.nodes["active_param1"] = await active_obj.add_variable(idx, "Param1", 0.0)
        self.nodes["active_param2"] = await active_obj.add_variable(idx, "Param2", 0.0)
        self.nodes["active_state"] = await active_obj.add_variable(idx, "State", 0)
        self.nodes["active_progress"] = await active_obj.add_variable(idx, "ProgressPct", 0.0)
        self.nodes["active_blocking"] = await active_obj.add_variable(idx, "BlockingCondition", "")
        self.nodes["active_start"] = await active_obj.add_variable(idx, "StartTimeMs", 0)
        self.nodes["active_last_update"] = await active_obj.add_variable(idx, "LastUpdateTimeMs", 0)

        for node in [
            self.operator_req.sequence_id,
            self.operator_req.command,
            self.operator_req.param1,
            self.operator_req.param2,
            self.operator_req.execute,
            self.remote_req.sequence_id,
            self.remote_req.command,
            self.remote_req.param1,
            self.remote_req.param2,
            self.remote_req.execute,
        ]:
            await node.set_writable()

    async def _add_command_branch(self, idx: int, parent: Any) -> tuple[RequestNodes, ResponseNodes]:
        req = await parent.add_object(idx, "Req")
        rsp = await parent.add_object(idx, "Rsp")

        req_nodes = RequestNodes(
            sequence_id=await req.add_variable(idx, "SequenceId", 0),
            command=await req.add_variable(idx, "Command", 0),
            param1=await req.add_variable(idx, "Param1", 0.0),
            param2=await req.add_variable(idx, "Param2", 0.0),
            execute=await req.add_variable(idx, "Execute", False),
        )

        rsp_nodes = ResponseNodes(
            ack_sequence_id=await rsp.add_variable(idx, "AckSequenceId", 0),
            status=await rsp.add_variable(idx, "Status", 0),
            reject_reason=await rsp.add_variable(idx, "RejectReason", ""),
            last_update_time_ms=await rsp.add_variable(idx, "LastUpdateTimeMs", 0),
        )

        return req_nodes, rsp_nodes

    async def start(self) -> None:
        await self.server.start()
        LOG.info("OPC UA gateway running at %s", ENDPOINT)
        try:
            await self._loop()
        finally:
            await self.server.stop()
            if self.http_session:
                await self.http_session.close()

    async def _loop(self) -> None:
        while True:
            try:
                snapshot = await self._fetch_snapshot()
                if snapshot is not None:
                    await self._sync_snapshot(snapshot)
                    await self._process_command("operator", self.operator_req, self.operator_rsp)
                    await self._process_command("remote", self.remote_req, self.remote_rsp)
            except Exception as exc:
                LOG.exception("Gateway sync loop failed: %s", exc)
            await asyncio.sleep(POLL_INTERVAL_SEC)

    async def _fetch_snapshot(self) -> Dict[str, Any] | None:
        assert self.http_session is not None
        url = f"{BACKEND_BASE_URL}/api/snapshot"
        async with self.http_session.get(url) as response:
            if response.status != 200:
                LOG.warning("snapshot request failed status=%s", response.status)
                return None
            return await response.json()

    async def _sync_snapshot(self, snapshot: Dict[str, Any]) -> None:
        diagnostics = snapshot["diagnostics"]
        mtp_modes = snapshot["mtp_modes"]
        mtp_state = snapshot["mtp_state_machine"]
        active = snapshot["active_command"]

        await self.nodes["endpoint_url"].write_value(diagnostics["endpoint_url"])
        await self.nodes["security_mode"].write_value(diagnostics["active_security_mode"])
        await self.nodes["connected_clients"].write_value(diagnostics["connected_client_count"])
        await self.nodes["subscription_count"].write_value(diagnostics["subscription_count"])
        await self.nodes["publish_rate"].write_value(float(diagnostics["publishing_rate_hz"]))
        await self.nodes["last_rejected"].write_value(diagnostics.get("last_rejected_command", ""))
        await self.nodes["last_error"].write_value(diagnostics.get("last_error", ""))

        await self.nodes["pressure_pa"].write_value(float(snapshot["pressure_pa"]))
        await self.nodes["temperature_k"].write_value(float(snapshot["temperature_k"]))
        await self.nodes["o2_percent"].write_value(float(snapshot["o2_percent"]))
        await self.nodes["inner_door_pct"].write_value(float(snapshot["inner_door_position_pct"]))
        await self.nodes["outer_door_pct"].write_value(float(snapshot["outer_door_position_pct"]))
        await self.nodes["pump_on"].write_value(bool(snapshot["pump_on"]))
        await self.nodes["equalize_valve_pct"].write_value(float(snapshot["equalize_valve_pct"]))
        await self.nodes["vent_valve_pct"].write_value(float(snapshot["vent_valve_pct"]))
        await self.nodes["alarm_summary"].write_value(snapshot["alarms"]["alarm_summary"])

        await self.nodes["operator_control_enabled"].write_value(
            bool(snapshot["permissions"]["operator_control_enabled"])
        )
        await self.nodes["remote_control_enabled"].write_value(
            bool(snapshot["permissions"]["remote_control_enabled"])
        )
        await self.nodes["leak_rate_nominal"].write_value(float(snapshot["leak_rate_nominal"]))

        await self.nodes["operation_mode"].write_value(mtp_modes["operation_mode"])
        await self.nodes["source_mode"].write_value(mtp_modes["source_mode"])
        await self.nodes["command_en"].write_value(bool(mtp_modes["command_en"]))
        await self.nodes["command_en_reason"].write_value(mtp_modes["command_en_reason"])

        await self.nodes["current_state"].write_value(mtp_state["current_state"])
        await self.nodes["time_in_state_sec"].write_value(float(mtp_state["time_in_state_sec"]))
        await self.nodes["transition_active"].write_value(bool(mtp_state["transition_active"]))
        await self.nodes["blocking_condition"].write_value(mtp_state["blocking_condition"])

        await self.nodes["service_name"].write_value(
            snapshot["mtp_runtime"]["service_information"]["service_name"]
        )
        await self.nodes["active_procedure"].write_value(
            snapshot["mtp_runtime"]["service_information"]["active_procedure"]
        )
        await self.nodes["health_state"].write_value(
            snapshot["mtp_runtime"]["pea_information_label"]["health_state"]
        )

        await self.nodes["active_command"].write_value(command_name_to_id(active["command"]))
        await self.nodes["active_source"].write_value(active["source"])
        await self.nodes["active_sequence"].write_value(int(active["sequence_id"]))
        await self.nodes["active_param1"].write_value(float(active["param1"]))
        await self.nodes["active_param2"].write_value(float(active["param2"]))
        await self.nodes["active_state"].write_value(status_name_to_id(active["state"]))
        await self.nodes["active_progress"].write_value(float(active["progress_pct"]))
        await self.nodes["active_blocking"].write_value(active["blocking_condition"])
        await self.nodes["active_start"].write_value(int(active["start_time_ms"]))
        await self.nodes["active_last_update"].write_value(int(active["last_update_time_ms"]))

        await self._sync_rsp(snapshot["operator_channel"]["rsp"], self.operator_rsp)
        await self._sync_rsp(snapshot["remote_channel"]["rsp"], self.remote_rsp)

    async def _sync_rsp(self, rsp_payload: Dict[str, Any], rsp_nodes: ResponseNodes) -> None:
        await rsp_nodes.ack_sequence_id.write_value(int(rsp_payload["ack_sequence_id"]))
        await rsp_nodes.status.write_value(status_name_to_id(rsp_payload["status"]))
        await rsp_nodes.reject_reason.write_value(rsp_payload["reject_reason"])
        await rsp_nodes.last_update_time_ms.write_value(int(rsp_payload["last_update_time_ms"]))

    async def _process_command(
        self,
        source: str,
        req_nodes: RequestNodes,
        rsp_nodes: ResponseNodes,
    ) -> None:
        execute = bool(await req_nodes.execute.read_value())
        if execute and not self.last_execute_state[source]:
            sequence_id = int(await req_nodes.sequence_id.read_value())
            command_id = int(await req_nodes.command.read_value())
            param1 = float(await req_nodes.param1.read_value())
            param2 = float(await req_nodes.param2.read_value())

            command_name = COMMAND_ID_TO_NAME.get(command_id)
            if command_name is None:
                await rsp_nodes.ack_sequence_id.write_value(sequence_id)
                await rsp_nodes.status.write_value(2)
                await rsp_nodes.reject_reason.write_value(f"Unknown command id: {command_id}")
                await rsp_nodes.last_update_time_ms.write_value(int(time.time() * 1000))
                await req_nodes.execute.write_value(False)
                self.last_execute_state[source] = False
                return

            payload = {
                "sequence_id": sequence_id,
                "command": command_name,
                "param1": param1,
                "param2": param2,
                "execute": True,
            }
            response = await self._write_backend_command(source, payload)

            if response is None:
                await rsp_nodes.ack_sequence_id.write_value(sequence_id)
                await rsp_nodes.status.write_value(2)
                await rsp_nodes.reject_reason.write_value("Gateway failed to forward request")
                await rsp_nodes.last_update_time_ms.write_value(int(time.time() * 1000))
            else:
                await rsp_nodes.ack_sequence_id.write_value(int(response.get("ack_sequence_id", sequence_id)))
                await rsp_nodes.status.write_value(status_name_to_id(response.get("status", "REJECTED")))
                await rsp_nodes.reject_reason.write_value(response.get("reject_reason", ""))
                await rsp_nodes.last_update_time_ms.write_value(
                    int(response.get("last_update_time_ms", int(time.time() * 1000)))
                )

            await req_nodes.execute.write_value(False)
            execute = False

        self.last_execute_state[source] = execute

    async def _write_backend_command(self, source: str, payload: Dict[str, Any]) -> Dict[str, Any] | None:
        assert self.http_session is not None
        url = f"{BACKEND_BASE_URL}/api/commands/{source}/write"

        try:
            async with self.http_session.post(url, data=json.dumps(payload)) as response:
                if response.status != 200:
                    LOG.warning("command forward failed source=%s status=%s", source, response.status)
                    return None
                body = await response.json()
        except Exception as exc:
            LOG.warning("command forward exception: %s", exc)
            return None

        reset_payload = dict(payload)
        reset_payload["execute"] = False

        try:
            async with self.http_session.post(url, data=json.dumps(reset_payload)):
                pass
        except Exception:
            LOG.debug("execute reset write failed for source=%s", source)

        return body


def command_name_to_id(name: str) -> int:
    for command_id, command_name in COMMAND_ID_TO_NAME.items():
        if command_name == name:
            return command_id
    return 0


def status_name_to_id(name: str) -> int:
    return STATUS_NAME_TO_ID.get(name, 2)


async def main() -> None:
    gateway = OpcuaGateway()
    await gateway.init()
    await gateway.start()


if __name__ == "__main__":
    asyncio.run(main())
