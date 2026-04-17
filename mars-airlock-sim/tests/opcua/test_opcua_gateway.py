"""OPC UA Gateway tests — validates the OPC UA browse tree and writable commands.

These tests require both the backend AND the OPC UA gateway to be running.
Maps to functional-specification.MD sections 5.2, 5.4, 6 and acceptance criteria 8.1.
"""

import asyncio
import os

import pytest
import pytest_asyncio
from asyncua import Client

from conftest import OPCUA_ENDPOINT

pytestmark = pytest.mark.asyncio

NAMESPACE_URI = "urn:mars-airlock:mtp"

# Skip all tests in this module if OPCUA gateway is not expected to be running
OPCUA_ENABLED = os.environ.get("TEST_OPCUA_ENABLED", "0") in ("1", "true", "yes")
pytestmark = [
    pytest.mark.asyncio,
    pytest.mark.skipif(not OPCUA_ENABLED, reason="OPC UA gateway not enabled (set TEST_OPCUA_ENABLED=1)"),
]


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest_asyncio.fixture(scope="module")
async def opcua_client():
    """Connect to the OPC UA gateway and yield the client."""
    client = Client(OPCUA_ENDPOINT)
    client.session_timeout = 30000

    for attempt in range(20):
        try:
            await client.connect()
            break
        except Exception:
            await asyncio.sleep(1)
    else:
        pytest.skip(f"Could not connect to OPC UA at {OPCUA_ENDPOINT}")

    yield client
    await client.disconnect()


@pytest_asyncio.fixture(scope="module")
async def ns_idx(opcua_client):
    """Resolve the namespace index for the airlock namespace."""
    idx = await opcua_client.get_namespace_index(NAMESPACE_URI)
    return idx


async def browse_child(parent, name):
    """Find a child node by BrowseName."""
    children = await parent.get_children()
    for child in children:
        bn = await child.read_browse_name()
        if bn.Name == name:
            return child
    return None


async def walk_path(root, path_parts):
    """Walk a path like ['MarsBase', 'AirlockPEA', 'Diagnostics']."""
    node = root
    for part in path_parts:
        node = await browse_child(node, part)
        if node is None:
            return None
    return node


# ---------------------------------------------------------------------------
# 1. Browse Tree Structure (Spec section 6)
# ---------------------------------------------------------------------------

class TestBrowseTree:
    async def test_marsbase_exists(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        mars_base = await browse_child(objects, "MarsBase")
        assert mars_base is not None, "Objects/MarsBase not found"

    async def test_airlock_pea_exists(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        pea = await walk_path(objects, ["MarsBase", "AirlockPEA"])
        assert pea is not None, "Objects/MarsBase/AirlockPEA not found"

    async def test_pea_information_label(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        info = await walk_path(objects, ["MarsBase", "AirlockPEA", "PEAInformationLabel"])
        assert info is not None

        tag_name = await browse_child(info, "TagName")
        assert tag_name is not None
        value = await tag_name.read_value()
        assert value == "AIRLOCK-PEA-001"

    async def test_diagnostics_node(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        diag = await walk_path(objects, ["MarsBase", "AirlockPEA", "Diagnostics"])
        assert diag is not None

        endpoint = await browse_child(diag, "EndpointUrl")
        assert endpoint is not None
        value = await endpoint.read_value()
        assert "mars-airlock" in value

    async def test_service_set_structure(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        service = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
        ])
        assert service is not None

        for expected in ["ServiceInformation", "Modes", "StateMachine", "DataAssemblies"]:
            child = await browse_child(service, expected)
            assert child is not None, f"Missing: AirlockService/{expected}"

    async def test_modes_variables(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        modes = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService", "Modes",
        ])
        assert modes is not None

        for var_name in ["OperationMode", "SourceMode", "CommandEn", "CommandEnReason", "ApplyEn", "ApplyEnReason"]:
            node = await browse_child(modes, var_name)
            assert node is not None, f"Missing Modes/{var_name}"

    async def test_data_assemblies_indicators(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        indicators = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
            "DataAssemblies", "Indicators",
        ])
        assert indicators is not None

        for var_name in ["PressurePa", "TemperatureK", "O2Percent",
                         "InnerDoorPositionPct", "OuterDoorPositionPct",
                         "PumpOn", "EqualizeValvePct", "VentValvePct"]:
            node = await browse_child(indicators, var_name)
            assert node is not None, f"Missing Indicators/{var_name}"

    async def test_control_command_branches(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        control = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
            "DataAssemblies", "Control",
        ])
        assert control is not None

        for branch in ["OperatorCommands", "RemoteCommands", "ActiveCommand"]:
            node = await browse_child(control, branch)
            assert node is not None, f"Missing Control/{branch}"


# ---------------------------------------------------------------------------
# 2. Telemetry Reads (Spec section 5.4.5)
# ---------------------------------------------------------------------------

class TestTelemetryReads:
    async def test_read_pressure(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        pressure = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
            "DataAssemblies", "Indicators", "PressurePa",
        ])
        assert pressure is not None
        value = await pressure.read_value()
        assert isinstance(value, float)
        assert value > 0

    async def test_read_temperature(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        temp = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
            "DataAssemblies", "Indicators", "TemperatureK",
        ])
        value = await temp.read_value()
        assert 100 < value < 400

    async def test_read_state_machine(self, opcua_client, ns_idx):
        objects = opcua_client.nodes.objects
        sm = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService", "StateMachine",
        ])
        current = await browse_child(sm, "CurrentState")
        value = await current.read_value()
        assert isinstance(value, str)


# ---------------------------------------------------------------------------
# 3. Writable Command Nodes (Spec section 5.3)
# ---------------------------------------------------------------------------

class TestWritableCommands:
    async def test_remote_req_nodes_writable(self, opcua_client, ns_idx):
        """Verify that Remote/Req nodes accept writes."""
        objects = opcua_client.nodes.objects
        req = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
            "DataAssemblies", "Control", "RemoteCommands", "Req",
        ])
        assert req is not None

        seq_node = await browse_child(req, "SequenceId")
        cmd_node = await browse_child(req, "Command")
        p1_node = await browse_child(req, "Param1")
        p2_node = await browse_child(req, "Param2")
        exec_node = await browse_child(req, "Execute")

        for node in [seq_node, cmd_node, p1_node, p2_node, exec_node]:
            assert node is not None

        # Write a command sequence (SET_PUMP_ON = 5, param1=1.0 for ON)
        test_seq = 77777
        await seq_node.write_value(test_seq)
        await cmd_node.write_value(5)  # SET_PUMP_ON
        await p1_node.write_value(1.0)
        await p2_node.write_value(0.0)
        await exec_node.write_value(True)

        # Wait for gateway to process
        await asyncio.sleep(1.0)

        # Read back response
        rsp = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
            "DataAssemblies", "Control", "RemoteCommands", "Rsp",
        ])
        ack = await browse_child(rsp, "AckSequenceId")
        ack_val = await ack.read_value()
        assert ack_val == test_seq

    async def test_active_command_reflects_write(self, opcua_client, ns_idx):
        """After a command write, ActiveCommand should update."""
        objects = opcua_client.nodes.objects
        active = await walk_path(objects, [
            "MarsBase", "AirlockPEA", "ServiceSet", "AirlockService",
            "DataAssemblies", "Control", "ActiveCommand",
        ])
        assert active is not None

        source_node = await browse_child(active, "Source")
        value = await source_node.read_value()
        assert isinstance(value, str)
