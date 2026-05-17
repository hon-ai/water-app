"""FastAPI app that exposes the analysis sidecar."""
from __future__ import annotations

import os
import time

from fastapi import FastAPI
from pydantic import BaseModel

from . import __version__

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
