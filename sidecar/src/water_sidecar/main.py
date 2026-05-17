"""FastAPI app that exposes the analysis sidecar."""
from __future__ import annotations

import os
import sys
import time

from fastapi import FastAPI
from pydantic import BaseModel

from . import __version__
from .routes import analyze as analyze_route

app = FastAPI(title="water-sidecar", version=__version__)
_started_at = time.time()


class HealthResponse(BaseModel):
    status: str
    version: str
    uptime_seconds: float
    pid: int


@app.get("/health", response_model=HealthResponse)
def health() -> HealthResponse:
    return HealthResponse(
        status="ready",
        version=__version__,
        uptime_seconds=time.time() - _started_at,
        pid=os.getpid(),
    )


app.include_router(analyze_route.router)


@app.on_event("startup")
async def announce_port() -> None:
    """Emit WATER_SIDECAR_PORT=NNNNN on stdout so the Rust core can read it."""
    # Discover the port from uvicorn server config. The actual server instance
    # is exposed by uvicorn in its lifespan; for v1 we instead require the
    # caller to supply --port and read it from argv.
    port = "0"
    for i, arg in enumerate(sys.argv):
        if arg == "--port" and i + 1 < len(sys.argv):
            port = sys.argv[i + 1]
            break
    print(f"WATER_SIDECAR_PORT={port}", flush=True)
