from fastapi.testclient import TestClient

from water_sidecar.main import app


def test_health_returns_ready() -> None:
    client = TestClient(app)
    r = client.get("/health")
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "ready"
    assert body["version"]
    assert body["pid"] > 0
