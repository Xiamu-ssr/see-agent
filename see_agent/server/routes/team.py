"""Team management API routes."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/teams", tags=["teams"])


# -------------------------------------------------------------------- #
# Request / response models
# -------------------------------------------------------------------- #


class CreateTeamRequest(BaseModel):
    name: str
    members: list[str]
    leader: str | None = None


class RunTeamRequest(BaseModel):
    task: str


# -------------------------------------------------------------------- #
# Routes
# -------------------------------------------------------------------- #


@router.post("/")
async def create_team(body: CreateTeamRequest) -> dict[str, Any]:
    """Create a new team."""
    from see_agent.team.definition import TeamDefinition

    team = TeamDefinition.create(
        body.name, body.members, leader=body.leader,
    )
    return {"id": team.id, "name": team.name, "status": team.status}


@router.get("/")
async def list_teams() -> list[dict[str, Any]]:
    """List all teams."""
    from see_agent.team.definition import TeamDefinition

    teams = TeamDefinition.list_all()
    return [
        {
            "id": t.id,
            "name": t.name,
            "members": t.members,
            "status": t.status,
        }
        for t in teams
    ]


@router.post("/{team_id}/run")
async def run_team(
    team_id: str, body: RunTeamRequest, request: Request,
) -> dict[str, Any]:
    """Run a task on a team."""
    from see_agent.team.definition import TeamDefinition
    from see_agent.team.manager import TeamManager

    try:
        team_def = TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    config = request.app.state.config
    manager = TeamManager(team_def, config)
    result = await manager.run(body.task)
    return {
        "team_id": result.team_id,
        "success": result.success,
        "summary": result.summary,
    }


@router.get("/{team_id}/status")
async def get_team_status(team_id: str) -> dict[str, Any]:
    """Get team status and task board."""
    from see_agent.config import TEAMS_DIR
    from see_agent.team.definition import TeamDefinition
    from see_agent.team.task_board import TaskBoard

    try:
        team = TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    board = TaskBoard(TEAMS_DIR / team_id)
    tasks = board.list_tasks()
    return {
        "id": team.id,
        "name": team.name,
        "members": team.members,
        "leader": team.leader,
        "status": team.status,
        "tasks": [
            {
                "id": t.id,
                "title": t.title,
                "status": t.status,
                "assigned_to": t.assigned_to,
            }
            for t in tasks
        ],
    }


@router.post("/{team_id}/stop")
async def stop_team(team_id: str) -> dict[str, str]:
    """Stop a running team."""
    from see_agent.team.definition import TeamDefinition

    try:
        team = TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    team.status = "stopped"
    team.save()
    return {"status": "stopped"}
