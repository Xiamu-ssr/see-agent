"""Miscellaneous API routes — tools, skills, config."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, Request
from pydantic import BaseModel

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["misc"])


class _NullEye:
    """Minimal eye for enumerating tools without real screen capture."""

    async def capture(self):  # type: ignore[override]  # noqa: ANN201
        raise NotImplementedError


@router.get("/tools")
async def list_tools(request: Request) -> list[dict[str, Any]]:
    """List all built-in tools (name, description, parameters)."""
    from see_agent.hand.tools import create_registry

    registry = create_registry(_NullEye())  # type: ignore[arg-type]
    results: list[dict[str, Any]] = []
    for name, tool in registry._tools.items():
        results.append({
            "name": name,
            "description": tool.description,
            "parameters": tool.parameters,
        })
    return results


@router.get("/skills")
async def list_skills(request: Request) -> list[dict[str, Any]]:
    """List loaded skills with gating status."""
    from see_agent.skill.loader import gate_skills, load_skills

    config = request.app.state.config
    skills_dirs = config.get("skills_dirs", [])
    skills = load_skills(skills_dirs) if skills_dirs else []
    gated = gate_skills(skills) if skills else []

    gated_names = {s.name for s in gated}
    results: list[dict[str, Any]] = []
    for s in skills:
        results.append({
            "name": s.name,
            "description": s.description,
            "available": s.name in gated_names,
        })
    return results


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
