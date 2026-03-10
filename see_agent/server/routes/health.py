"""Health-check route for the see-agent API."""

from __future__ import annotations

import logging

from fastapi import APIRouter

from see_agent.server.schemas import HealthResponse

logger = logging.getLogger(__name__)

router = APIRouter()


@router.get("/api/health")
async def health_check() -> HealthResponse:
    """Return a simple health status with the API version.

    Returns:
        A JSON object with ``status`` and ``version`` keys.
    """
    return HealthResponse(status="ok", version="0.1.0")
