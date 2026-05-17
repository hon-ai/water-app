# Water sidecar

The analysis sidecar — a small FastAPI process spawned by the Rust core. The
renderer never talks to this directly; only `water-core` does.

## Develop

```bash
cd sidecar
uv sync --extra dev
uv run uvicorn water_sidecar.main:app --port 0
```

## Test

```bash
uv run pytest
```
