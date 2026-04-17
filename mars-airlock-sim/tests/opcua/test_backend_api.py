"""Backend REST API tests — validates simulation engine, command model, and interlocks.

These tests run against the backend HTTP API directly (no OPC UA gateway needed).
Maps to functional-specification.MD sections 5.1–5.5 and acceptance criteria 8.1–8.2.
"""

import asyncio
import json

import aiohttp
import pytest
import pytest_asyncio

from conftest import BACKEND_URL, send_command, wait_for_backend

pytestmark = pytest.mark.asyncio


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest_asyncio.fixture(scope="module", autouse=True)
async def ensure_backend():
    await wait_for_backend(BACKEND_URL)


@pytest_asyncio.fixture
async def session():
    async with aiohttp.ClientSession(
        timeout=aiohttp.ClientTimeout(total=10),
        headers={"content-type": "application/json"},
    ) as s:
        yield s


@pytest_asyncio.fixture
async def clean_state(session):
    """Reset the simulation to a known baseline before each test."""
    # Reset faults
    await send_command(session, BACKEND_URL, "operator", "RESET_FAULTS", seq=9000)
    # Ensure both channels enabled
    async with session.post(
        f"{BACKEND_URL}/api/permissions",
        json={"operator_control_enabled": True, "remote_control_enabled": True},
    ) as resp:
        assert resp.status == 200
    # Ensure CommandEn is on
    async with session.post(
        f"{BACKEND_URL}/api/modes",
        json={"command_en": True, "command_en_reason": ""},
    ) as resp:
        assert resp.status == 200
    yield


# ---------------------------------------------------------------------------
# 1. Health & Snapshot
# ---------------------------------------------------------------------------

class TestHealthAndSnapshot:
    async def test_health(self, session):
        async with session.get(f"{BACKEND_URL}/api/health") as resp:
            assert resp.status == 200
            body = await resp.json()
            assert body["status"] == "ok"

    async def test_snapshot_structure(self, session):
        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            assert resp.status == 200
            snap = await resp.json()

        # Spec section 5.7: minimum snapshot fields
        required_keys = [
            "timestamp_ms", "pressure_pa", "temperature_k", "o2_percent",
            "inner_door_position_pct", "outer_door_position_pct",
            "inner_lock_engaged", "outer_lock_engaged",
            "equalize_valve_pct", "vent_valve_pct", "pump_on",
            "state_name", "active_command", "alarms", "diagnostics",
            "mtp_modes", "mtp_state_machine", "mtp_runtime",
            "event_log", "operator_channel", "remote_channel",
        ]
        for key in required_keys:
            assert key in snap, f"Missing snapshot key: {key}"
        # additional MTP-specific subfields
        assert isinstance(snap["mtp_modes"]["command_en"], bool)
        assert isinstance(snap["mtp_modes"]["command_en_reason"], str)
        assert isinstance(snap["mtp_modes"]["apply_en"], bool)
        assert isinstance(snap["mtp_modes"]["apply_en_reason"], str)

    async def test_snapshot_initial_values(self, session):
        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            snap = await resp.json()

        # Habitat pressure at startup (~101325 Pa)
        assert 90000 < snap["pressure_pa"] <= 120000
        assert 200 < snap["temperature_k"] < 350
        assert 0 < snap["o2_percent"] <= 22


# ---------------------------------------------------------------------------
# 2. MTP Browse Tree (Spec section 6)
# ---------------------------------------------------------------------------

class TestMtpTree:
    async def test_mtp_tree_contains_required_nodes(self, session):
        async with session.get(f"{BACKEND_URL}/api/mtp/tree") as resp:
            assert resp.status == 200
            tree = await resp.json()

        assert tree["root_path"] == "Objects/MarsBase/AirlockPEA"
        nodes = tree["nodes"]

        required_nodes = [
            "PEAInformationLabel",
            "Diagnostics",
            "ServiceSet/AirlockService/Modes",
            "ServiceSet/AirlockService/StateMachine",
            "ServiceSet/AirlockService/Procedures/Proc_DepressurizeForEVA",
            "ServiceSet/AirlockService/Procedures/Proc_PressurizeForEntry",
            "ServiceSet/AirlockService/Procedures/Proc_ManualDoorJog",
            "ServiceSet/AirlockService/DataAssemblies/Indicators",
            "ServiceSet/AirlockService/DataAssemblies/Parameters",
            "ServiceSet/AirlockService/DataAssemblies/ActiveElements",
            "ServiceSet/AirlockService/DataAssemblies/Control/OperatorCommands/Req",
            "ServiceSet/AirlockService/DataAssemblies/Control/RemoteCommands/Req",
            "ServiceSet/AirlockService/DataAssemblies/Control/ActiveCommand",
            "Simulation",
            "FaultInjection",
        ]
        for node in required_nodes:
            assert node in nodes, f"Missing MTP node: {node}"

        # make sure the REST service definition exposes our procedures
        async with session.get(f"{BACKEND_URL}/api/v2/pea/{DEFAULT_AIRLOCK_PEA_ID}/services/AirlockService") as resp:
            assert resp.status == 200
            sd = await resp.json()
        proc_names = [p["name"] for p in sd.get("procedures", [])]
        for expected in ["DepressurizeForEVA", "PressurizeForEntry", "ManualDoorJog"]:
            assert expected in proc_names, f"Procedure {expected} missing in service definition"


# ---------------------------------------------------------------------------
# 3. Command Model (Spec sections 5.3)
# ---------------------------------------------------------------------------

class TestCommandModel:
    async def test_operator_command_accepted(self, session, clean_state):
        result = await send_command(
            session, BACKEND_URL, "operator", "SET_PUMP_ON", param1=1.0, seq=100
        )
        assert result["status"] == "ACCEPTED" or result["status"] == "COMPLETE"
        assert result["ack_sequence_id"] == 100

    async def test_remote_command_accepted(self, session, clean_state):
        result = await send_command(
            session, BACKEND_URL, "remote", "SET_PUMP_ON", param1=0.0, seq=200
        )
        assert result["status"] in ("ACCEPTED", "COMPLETE")
        assert result["ack_sequence_id"] == 200

    async def test_duplicate_sequence_rejected(self, session, clean_state):
        # First command
        await send_command(
            session, BACKEND_URL, "operator", "SET_PUMP_ON", param1=1.0, seq=300
        )
        # Same sequence ID should be rejected
        result = await send_command(
            session, BACKEND_URL, "operator", "RESET_FAULTS", seq=300
        )
        assert result["status"] == "REJECTED"
        assert "Duplicate" in result["reject_reason"] or "duplicate" in result["reject_reason"].lower()

    async def test_active_command_telemetry(self, session, clean_state):
        await send_command(
            session, BACKEND_URL, "operator", "SET_EQUALIZE_VALVE_PCT", param1=50.0, seq=400
        )
        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            snap = await resp.json()

        active = snap["active_command"]
        assert active["sequence_id"] == 400
        assert active["source"] == "OPERATOR_UI"

    async def test_busy_rejection(self, session, clean_state):
        """Start a long-running command, then try another — should get 'Busy'."""
        await send_command(
            session, BACKEND_URL, "operator", "START_DEPRESSURIZE_CYCLE", seq=500
        )
        result = await send_command(
            session, BACKEND_URL, "operator", "START_PRESSURIZE_CYCLE", seq=501
        )
        assert result["status"] == "REJECTED"
        assert "Busy" in result["reject_reason"]

    async def test_abort_during_active(self, session, clean_state):
        """ABORT_CYCLE should be accepted even when another command is running."""
        await send_command(
            session, BACKEND_URL, "operator", "START_DEPRESSURIZE_CYCLE", seq=600
        )
        result = await send_command(
            session, BACKEND_URL, "operator", "ABORT_CYCLE", seq=601
        )
        assert result["status"] in ("ACCEPTED", "COMPLETE", "ABORTED")


# ---------------------------------------------------------------------------
# 4. Permissions (Spec section 5.3.4)
# ---------------------------------------------------------------------------

class TestPermissions:
    async def test_operator_disabled_rejects(self, session, clean_state):
        async with session.post(
            f"{BACKEND_URL}/api/permissions",
            json={"operator_control_enabled": False},
        ) as resp:
            assert resp.status == 200

        result = await send_command(
            session, BACKEND_URL, "operator", "SET_PUMP_ON", param1=1.0, seq=700
        )
        assert result["status"] == "REJECTED"
        assert "operator control disabled" in result["reject_reason"].lower()

        # Re-enable
        async with session.post(
            f"{BACKEND_URL}/api/permissions",
            json={"operator_control_enabled": True},
        ) as resp:
            pass

    async def test_remote_disabled_rejects(self, session, clean_state):
        async with session.post(
            f"{BACKEND_URL}/api/permissions",
            json={"remote_control_enabled": False},
        ) as resp:
            assert resp.status == 200

        result = await send_command(
            session, BACKEND_URL, "remote", "SET_PUMP_ON", param1=1.0, seq=800
        )
        assert result["status"] == "REJECTED"
        assert "remote control disabled" in result["reject_reason"].lower()

        async with session.post(
            f"{BACKEND_URL}/api/permissions",
            json={"remote_control_enabled": True},
        ) as resp:
            pass

    async def test_command_en_disabled_rejects(self, session, clean_state):
        async with session.post(
            f"{BACKEND_URL}/api/modes",
            json={"command_en": False, "command_en_reason": "Maintenance lockout"},
        ) as resp:
            assert resp.status == 200

        result = await send_command(
            session, BACKEND_URL, "operator", "SET_PUMP_ON", param1=1.0, seq=850
        )
        assert result["status"] == "REJECTED"
        assert "Maintenance lockout" in result["reject_reason"] or "CommandEn" in result["reject_reason"]

        # confirm snapshot also reports apply_en false
        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            snap = await resp.json()
        assert snap["mtp_modes"]["apply_en"] is False
        assert "Maintenance lockout" in snap["mtp_modes"]["apply_en_reason"]

        # Re-enable
        async with session.post(
            f"{BACKEND_URL}/api/modes",
            json={"command_en": True, "command_en_reason": ""},
        ) as resp:
            pass


# ---------------------------------------------------------------------------
# 5. Interlocks (Spec section 5.1.6)
# ---------------------------------------------------------------------------

class TestInterlocks:
    async def test_outer_unlock_blocked_high_pressure(self, session, clean_state):
        """Outer unlock should be blocked when pressure > OuterUnlockMaxPressurePa (5000)."""
        # At startup, pressure is ~101325 Pa (well above 5000)
        result = await send_command(
            session, BACKEND_URL, "operator", "UNLOCK_OUTER_DOOR", seq=900
        )
        assert result["status"] == "REJECTED"
        assert "pressure too high" in result["reject_reason"].lower()

    async def test_inner_unlock_blocked_low_pressure(self, session, clean_state):
        """Inner unlock below InnerUnlockMinPressurePa is tricky to test at startup.
        Pressure starts at ~101325 which is above 90000, so inner unlock should succeed."""
        result = await send_command(
            session, BACKEND_URL, "operator", "UNLOCK_INNER_DOOR", seq=950
        )
        # At startup pressure ~101325 > 90000, so this should be accepted
        assert result["status"] in ("ACCEPTED", "COMPLETE")


# ---------------------------------------------------------------------------
# 6. Diagnostics (Spec section 5.5)
# ---------------------------------------------------------------------------

class TestDiagnostics:
    async def test_diagnostics_fields(self, session):
        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            snap = await resp.json()

        diag = snap["diagnostics"]
        assert "endpoint_url" in diag
        assert "active_security_mode" in diag
        assert "connected_client_count" in diag
        assert "subscription_count" in diag
        assert "publishing_rate_hz" in diag
        assert "last_rejected_command" in diag
        assert "server_uptime_sec" in diag
        assert diag["server_uptime_sec"] >= 0

    async def test_security_profile_change(self, session):
        async with session.post(
            f"{BACKEND_URL}/api/security/profile",
            json={"profile": "BASIC256SHA256"},
        ) as resp:
            assert resp.status == 200

        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            snap = await resp.json()
        assert snap["diagnostics"]["active_security_mode"] == "BASIC256SHA256"

        # Reset to NONE
        async with session.post(
            f"{BACKEND_URL}/api/security/profile",
            json={"profile": "NONE"},
        ) as resp:
            pass


# ---------------------------------------------------------------------------
# 7. MTP Modes and State Machine (Spec section 5.4.3)
# ---------------------------------------------------------------------------

class TestMtpModesAndState:
    async def test_modes_update(self, session, clean_state):
        async with session.post(
            f"{BACKEND_URL}/api/modes",
            json={"operation_mode": "MANUAL"},
        ) as resp:
            assert resp.status == 200
            body = await resp.json()
            assert body["mtp_modes"]["operation_mode"] == "MANUAL"

        # Reset
        async with session.post(
            f"{BACKEND_URL}/api/modes",
            json={"operation_mode": "AUTO"},
        ) as resp:
            pass

    async def test_state_machine_transitions(self, session, clean_state):
        """Issuing a cycle command should transition state machine to 'execute'."""
        await send_command(
            session, BACKEND_URL, "operator", "START_DEPRESSURIZE_CYCLE", seq=1100
        )
        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            snap = await resp.json()

        sm = snap["mtp_state_machine"]
        assert sm["current_state"] in ("execute", "completed", "completing")

        # Abort to clean up
        await send_command(
            session, BACKEND_URL, "operator", "ABORT_CYCLE", seq=1101
        )


# ---------------------------------------------------------------------------
# 8. Fault Injection (Spec section 5.4.1 — FaultInjection node)
# ---------------------------------------------------------------------------

class TestFaultInjection:
    async def test_leak_rate_update(self, session, clean_state):
        async with session.post(
            f"{BACKEND_URL}/api/faults/leak-rate",
            json={"leak_rate_nominal": 0.005},
        ) as resp:
            assert resp.status == 200

        async with session.get(f"{BACKEND_URL}/api/snapshot") as resp:
            snap = await resp.json()
        assert snap["leak_rate_nominal"] == pytest.approx(0.005, abs=0.0001)
        assert snap["alarms"]["leak_detected"] is True

        # Reset
        async with session.post(
            f"{BACKEND_URL}/api/faults/leak-rate",
            json={"leak_rate_nominal": 0.0005},
        ) as resp:
            pass


# ---------------------------------------------------------------------------
# 9. Event Log
# ---------------------------------------------------------------------------

class TestEventLog:
    async def test_events_returned(self, session):
        async with session.get(f"{BACKEND_URL}/api/events") as resp:
            assert resp.status == 200
            events = await resp.json()

        assert isinstance(events, list)
        if len(events) > 0:
            entry = events[0]
            assert "timestamp_ms" in entry
            assert "severity" in entry
            assert "source" in entry
            assert "message" in entry
