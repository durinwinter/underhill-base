"""i3X compatibility tests using the shared Focus demo client.

This test suite imports `/home/earthling/Documents/Focus/test_client.py`
and uses its async API helpers against the Underhill `/api/v1` endpoints.
"""

import importlib.util
from pathlib import Path

import pytest
import pytest_asyncio

from conftest import BACKEND_URL, wait_for_backend

pytestmark = pytest.mark.asyncio


def _load_demo_client_module():
    focus_root = Path(__file__).resolve().parents[4]
    demo_client_path = focus_root / "test_client.py"
    if not demo_client_path.exists():
        raise FileNotFoundError(f"Demo client not found at {demo_client_path}")

    spec = importlib.util.spec_from_file_location("focus_i3x_demo_client", demo_client_path)
    if spec is None or spec.loader is None:
        raise ImportError(f"Unable to load module spec from {demo_client_path}")

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def demo_client():
    return _load_demo_client_module()


@pytest_asyncio.fixture(scope="module", autouse=True)
async def ensure_backend_ready():
    await wait_for_backend(BACKEND_URL)


@pytest.fixture(scope="module")
def i3x_base_url():
    return f"{BACKEND_URL}/api/v1"


async def test_demo_client_get_namespaces(demo_client, i3x_base_url):
    namespaces = await demo_client.get_namespaces(i3x_base_url)
    assert isinstance(namespaces, list)
    uris = {item.get("uri") for item in namespaces}
    assert "https://underhill.murph/ns/pea" in uris


async def test_demo_client_get_object_types(demo_client, i3x_base_url):
    object_types = await demo_client.get_object_types(i3x_base_url)
    assert isinstance(object_types, list)
    element_ids = {item.get("elementId") for item in object_types}
    assert {"BaseEquipment", "PEAType", "ServiceType"}.issubset(element_ids)

    filtered = await demo_client.get_object_types(i3x_base_url, "https://underhill.murph/ns/pea")
    assert isinstance(filtered, list)
    assert filtered, "Expected at least one object type in Underhill namespace"
    assert all(
        item.get("namespaceUri") == "https://underhill.murph/ns/pea" for item in filtered
    )


async def test_demo_client_get_relationship_types(demo_client, i3x_base_url):
    relationship_types = await demo_client.get_relationship_types(i3x_base_url)
    assert isinstance(relationship_types, list)
    element_ids = {item.get("elementId") for item in relationship_types}
    assert {"HasParent", "HasChildren"}.issubset(element_ids)


async def test_demo_client_get_objects(demo_client, i3x_base_url):
    objects = await demo_client.get_objects(i3x_base_url, include_metadata=False)
    assert isinstance(objects, list)
    element_ids = {item.get("elementId") for item in objects}
    assert {
        "underhill-base",
        "AIRLOCK-PEA-001",
        "ECLSS-PEA-001",
        "SABATIER-PEA-001",
    }.issubset(element_ids)
