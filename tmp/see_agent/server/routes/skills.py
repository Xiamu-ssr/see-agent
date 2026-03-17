"""API routes — skills listing and installation."""

from __future__ import annotations

import logging
import subprocess

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel

from see_agent.config import SKILLS_DIR
from see_agent.server.schemas import SkillInfo, SkillInstallResponse

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["skills"])


@router.get("/skills")
async def list_skills(request: Request) -> list[SkillInfo]:
    """List loaded skills with gating status."""
    from see_agent.skill.loader import gate_skills, load_skills

    config = request.app.state.config
    skills_dirs = config.get("skills", {}).get("dirs", [])
    skills = load_skills(skills_dirs) if skills_dirs else []
    gated = gate_skills(skills) if skills else []

    gated_names = {s.name for s in gated}
    results: list[SkillInfo] = []
    for s in skills:
        results.append(SkillInfo(
            name=s.name,
            description=s.description,
            available=s.name in gated_names,
        ))
    return results


class InstallSkillRequest(BaseModel):
    name: str


@router.post("/skills/install")
async def install_skill(body: InstallSkillRequest) -> SkillInstallResponse:
    """Install a skill from ClawHub."""
    try:
        result = subprocess.run(
            ["clawhub", "install", body.name, "--target", str(SKILLS_DIR)],
            capture_output=True,
            text=True,
            timeout=60,
        )
    except FileNotFoundError:
        logger.warning("clawhub binary not found")
        raise HTTPException(
            status_code=500,
            detail="clawhub is not installed. Run `pip install clawhub` first.",
        )
    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=504, detail="Install timed out.")
    if result.returncode != 0:
        raise HTTPException(status_code=400, detail=f"Install failed: {result.stderr}")
    return SkillInstallResponse(status="ok", name=body.name)


@router.get("/agents/{agent_id}/skills")
async def get_agent_skills(agent_id: str, request: Request):
    """Return skill list with per-agent disabled status."""
    from see_agent.agent.definition import AgentDefinition
    from see_agent.skill.loader import load_skills

    try:
        defn = AgentDefinition.load(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    config = request.app.state.config
    skills_dirs = config.get("skills", {}).get("dirs", [])
    skills = load_skills(skills_dirs) if skills_dirs else []
    disabled: list[str] = defn.skills.get("disabled", [])

    return {
        "skills": [
            {"name": s.name, "description": s.description, "disabled": s.name in disabled}
            for s in skills
        ],
        "disabled": disabled,
    }


class UpdateAgentSkillsRequest(BaseModel):
    disabled: list[str]


@router.put("/agents/{agent_id}/skills")
async def update_agent_skills(agent_id: str, body: UpdateAgentSkillsRequest):
    """Update the agent's skills.disabled list."""
    from see_agent.agent.definition import AgentDefinition

    try:
        defn = AgentDefinition.load(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    defn.skills["disabled"] = body.disabled
    defn.save()
    return {"status": "ok", "disabled": body.disabled}
