import os
import asyncio
import pytest
import aiohttp

BACKEND_URL = os.environ.get("TEST_BACKEND_URL", "http://127.0.0.1:8080")
OPCUA_ENDPOINT = os.environ.get("TEST_OPCUA_ENDPOINT", "opc.tcp://127.0.0.1:4841/mars-airlock")


@pytest.fixture(scope="session")
def event_loop():
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="session")
def backend_url():
    return BACKEND_URL


@pytest.fixture(scope="session")
def opcua_endpoint():
    return OPCUA_ENDPOINT


@pytest.fixture
async def http_session():
    async with aiohttp.ClientSession(
        timeout=aiohttp.ClientTimeout(total=10),
        headers={"content-type": "application/json"},
    ) as session:
        yield session


async def send_command(session, backend_url, source, command, param1=0, param2=0, seq=None):
    """Helper to send a command via the REST API with rising-edge handshake."""
    if seq is None:
        seq = int(asyncio.get_event_loop().time() * 1000) % 2**31

    payload = {
        "sequence_id": seq,
        "command": command,
        "param1": param1,
        "param2": param2,
        "execute": True,
    }
    url = f"{backend_url}/api/commands/{source}/write"
    async with session.post(url, json=payload) as resp:
        assert resp.status == 200, f"Command write failed: {resp.status}"
        result = await resp.json()

    # Reset execute (falling edge)
    payload["execute"] = False
    async with session.post(url, json=payload) as resp:
        pass

    return result


async def wait_for_backend(backend_url, timeout=30):
    """Poll backend health until ready."""
    async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=5)) as session:
        for _ in range(timeout * 2):
            try:
                async with session.get(f"{backend_url}/api/health") as resp:
                    if resp.status == 200:
                        return True
            except Exception:
                pass
            await asyncio.sleep(0.5)
    raise TimeoutError(f"Backend at {backend_url} not ready after {timeout}s")
