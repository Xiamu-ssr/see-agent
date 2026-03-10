"""API routes — dashboard summary."""

from __future__ import annotations

import logging

from fastapi import APIRouter, Request

from see_agent.server.schemas import DashboardResponse

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["dashboard"])


@router.get("/dashboard")
async def get_dashboard(request: Request) -> DashboardResponse:
    """Return dashboard summary statistics."""
    from see_agent.agent.definition import AgentDefinition
    from see_agent.config import TEAMS_DIR
    from see_agent.team.definition import TeamDefinition
    from see_agent.team.task_board import TaskBoard

    teams = TeamDefinition.list_all()
    all_agents = AgentDefinition.list_all_global()

    teams_by_status: dict[str, int] = {}
    for t in teams:
        teams_by_status[t.status] = teams_by_status.get(t.status, 0) + 1

    agents_in_team = sum(1 for _, tid in all_agents if tid is not None)
    agents_idle = sum(1 for _, tid in all_agents if tid is None)

    total_tasks = 0
    tasks_by_status: dict[str, int] = {}
    for t in teams:
        board = TaskBoard(TEAMS_DIR / t.id)
        for task in board.list_tasks():
            total_tasks += 1
            tasks_by_status[task.status] = tasks_by_status.get(task.status, 0) + 1

    return DashboardResponse(
        teams_count=len(teams),
        teams_by_status=teams_by_status,
        agents_in_team=agents_in_team,
        agents_idle=agents_idle,
        total_tasks=total_tasks,
        tasks_by_status=tasks_by_status,
    )
