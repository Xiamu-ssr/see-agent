"""API routes — configuration management."""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel

from see_agent.server.schemas import StatusResponse

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
) -> StatusResponse:
    """Update global config."""
    from see_agent.config import _deep_merge, save_config

    current = request.app.state.config
    merged = _deep_merge(current, body.config)
    save_config(merged)
    request.app.state.config = merged
    return StatusResponse(status="updated")


_SCHEMA_DIR = Path(__file__).parent.parent.parent / "schemas"


@router.get("/schemas/{schema_type}")
async def get_schema(schema_type: str) -> dict[str, Any]:
    """Return a JSON schema by type (config, agent, team)."""
    schema_path = _SCHEMA_DIR / f"{schema_type}.schema.json"
    if not schema_path.exists():
        raise HTTPException(status_code=404, detail=f"Schema not found: {schema_type}")
    return json.loads(schema_path.read_text(encoding="utf-8"))
