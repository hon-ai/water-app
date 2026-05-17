"""POST /analyze — stub deterministic metrics for M1."""
from __future__ import annotations

import hashlib
from typing import Literal

from fastapi import APIRouter
from pydantic import BaseModel, Field

router = APIRouter()


class AnalyzeRequest(BaseModel):
    text: str = Field(min_length=1)
    scene_id: str


class AnalyzeResponse(BaseModel):
    word_count: int
    flow: float
    coherence: float
    engagement: float
    divergence: float
    pace: float
    intensity: float
    valence: float
    status: Literal["flow", "drift", "blocked", "normal", "warming_up"]


def _stable_score(seed: bytes, offset: int) -> float:
    """Deterministic 0..1 score from a hash of the text. Same input → same
    output. Suitable as a placeholder until real models land in M2/M5."""
    h = hashlib.sha256(seed).digest()
    return ((h[offset % 32] / 255.0) * 0.6) + 0.2  # confined to 0.2..0.8


@router.post("/analyze", response_model=AnalyzeResponse)
def analyze(req: AnalyzeRequest) -> AnalyzeResponse:
    word_count = len(req.text.split())
    if word_count < 5:
        return AnalyzeResponse(
            word_count=word_count,
            flow=0.5, coherence=0.5, engagement=0.5, divergence=0.0,
            pace=0.5, intensity=0.5, valence=0.5,
            status="warming_up",
        )

    seed = req.text.encode("utf-8")
    return AnalyzeResponse(
        word_count=word_count,
        flow=_stable_score(seed, 0),
        coherence=_stable_score(seed, 1),
        engagement=_stable_score(seed, 2),
        divergence=_stable_score(seed, 3) * 0.7,
        pace=_stable_score(seed, 4),
        intensity=_stable_score(seed, 5),
        valence=_stable_score(seed, 6),
        status="normal",
    )
