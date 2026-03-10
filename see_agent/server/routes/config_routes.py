"""API routes — configuration management."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, Request
from pydantic import BaseModel

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["config"])


@router.get("/config")
async def get_config(request: Request) -> dict[str, Any]:
    """Return global config (api_key masked)."""
    import copy

    config = copy.deepcopy(request.app.state.config)
    llm = config.get("llm", {})
    if llm.get("api_key"):
        key = llm["api_key"]
        llm["api_key"] = key[:4] + "****" + key[-4:] if len(key) > 8 else "****"
    return config


class UpdateConfigRequest(BaseModel):
    config: dict[str, Any]


@router.put("/config")
async def update_config(
    body: UpdateConfigRequest, request: Request,
) -> dict[str, str]:
    """Update global config."""
    from see_agent.config import _deep_merge, save_config

    current = request.app.state.config
    merged = _deep_merge(current, body.config)
    save_config(merged)
    request.app.state.config = merged
    return {"status": "updated"}
