"""Health-check route for the see-agent API."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter

logger = logging.getLogger(__name__)

router = APIRouter()


@router.get("/api/health")
async def health_check() -> dict[str, Any]:
    """Return a simple health status with the API version.

    Returns:
        A JSON object with ``status`` and ``version`` keys.
    """
    return {"status": "ok", "version": "0.1.0"}
