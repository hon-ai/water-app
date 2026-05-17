from fastapi.testclient import TestClient

from water_sidecar.main import app


def test_analyze_short_text_returns_warming_up_status() -> None:
    client = TestClient(app)
    r = client.post("/analyze", json={"text": "Hi.", "scene_id": "01H8X4"})
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "warming_up"
    assert body["word_count"] == 1


def test_analyze_with_paragraph_returns_metrics() -> None:
    client = TestClient(app)
    text = (
        "The fog rolled in low over the cliffs, swallowing the harbour "
        "lanterns one by one. Maren watched the last of them go."
    )
    r = client.post("/analyze", json={"text": text, "scene_id": "01H8X4"})
    assert r.status_code == 200
    body = r.json()
    assert body["word_count"] >= 10
    for k in ("flow", "coherence", "engagement", "divergence", "pace", "intensity", "valence"):
        assert 0.0 <= body[k] <= 1.0, f"{k} out of range"
    assert body["status"] in {"flow", "drift", "blocked", "normal", "warming_up"}


def test_analyze_rejects_empty_text() -> None:
    client = TestClient(app)
    r = client.post("/analyze", json={"text": "", "scene_id": "01H8X4"})
    assert r.status_code == 422
