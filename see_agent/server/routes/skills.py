"""API routes — skills listing."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, Request

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["skills"])


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
