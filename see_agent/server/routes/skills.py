"""API routes — skills listing and installation."""

from __future__ import annotations

import logging
import subprocess
from typing import Any

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel

from see_agent.config import SKILLS_DIR

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


class InstallSkillRequest(BaseModel):
    name: str


@router.post("/skills/install")
async def install_skill(body: InstallSkillRequest) -> dict[str, str]:
    """Install a skill from ClawHub."""
    result = subprocess.run(
        ["clawhub", "install", body.name, "--target", str(SKILLS_DIR)],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        raise HTTPException(status_code=400, detail=f"Install failed: {result.stderr}")
    return {"status": "ok", "name": body.name}
