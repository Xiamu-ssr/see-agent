"""Team management API routes."""

from __future__ import annotations

import json
import logging
from pathlib import Path

from fastapi import APIRouter, HTTPException, Query, Request
from pydantic import BaseModel

from see_agent.server.schemas import (
    AgentStatusResponse,
    MarkReadResponse,
    StatusResponse,
    TaskItem,
    TeamCreateResponse,
    TeamLogEntry,
    TeamMessage,
    TeamRunResponse,
    TeamStatus,
    TeamSummary,
    TeamUpdateResponse,
    UnreadResponse,
)

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


class UpdateTeamRequest(BaseModel):
    name: str | None = None
    members: list[str] | None = None
    leader: str | None = None
    screen_mode: str | None = None


class OwnerMessageRequest(BaseModel):
    to: str
    content: str


# -------------------------------------------------------------------- #
# Routes
# -------------------------------------------------------------------- #


@router.post("")
async def create_team(body: CreateTeamRequest) -> TeamCreateResponse:
    """Create a new team."""
    from see_agent.team.definition import TeamDefinition

    team = TeamDefinition.create(
        body.name, body.members, leader=body.leader,
    )
    return TeamCreateResponse(id=team.id, name=team.name, status=team.status)


@router.get("")
async def list_teams() -> list[TeamSummary]:
    """List all teams."""
    from see_agent.team.definition import TeamDefinition

    teams = TeamDefinition.list_all()
    return [
        TeamSummary(
            id=t.id,
            name=t.name,
            members=t.members,
            status=t.status,
        )
        for t in teams
    ]


@router.put("/{team_id}")
async def update_team(
    team_id: str, body: UpdateTeamRequest,
) -> TeamUpdateResponse:
    """Update an existing team definition."""
    from see_agent.team.definition import TeamDefinition

    try:
        team = TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    if body.name is not None:
        team.name = body.name
    if body.members is not None:
        team.members = body.members
    if body.leader is not None:
        team.leader = body.leader
    if body.screen_mode is not None:
        team.screen_mode = body.screen_mode

    team.save()
    return TeamUpdateResponse(
        id=team.id,
        name=team.name,
        members=team.members,
        status=team.status,
    )


@router.post("/{team_id}/run")
async def run_team(
    team_id: str, body: RunTeamRequest, request: Request,
) -> TeamRunResponse:
    """Run a task on a team."""
    from see_agent.team.definition import TeamDefinition
    from see_agent.team.manager import TeamManager

    try:
        team_def = TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    config = request.app.state.config
    manager = TeamManager(team_def, config)
    # Store manager for live message injection.
    request.app.state.team_managers[team_id] = manager
    try:
        result = await manager.run(body.task)
    finally:
        request.app.state.team_managers.pop(team_id, None)
    return TeamRunResponse(
        team_id=result.team_id,
        success=result.success,
        summary=result.summary,
    )


@router.get("/{team_id}/status")
async def get_team_status(team_id: str) -> TeamStatus:
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
    return TeamStatus(
        id=team.id,
        name=team.name,
        members=team.members,
        leader=team.leader,
        status=team.status,
        tasks=[
            TaskItem(
                id=t.id,
                title=t.title,
                status=t.status,
                assigned_to=t.assigned_to,
            )
            for t in tasks
        ],
    )


@router.post("/{team_id}/stop")
async def stop_team(team_id: str) -> StatusResponse:
    """Stop a running team."""
    from see_agent.team.definition import TeamDefinition

    try:
        team = TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    team.status = "stopped"
    team.save()
    return StatusResponse(status="stopped")


# -------------------------------------------------------------------- #
# Owner communication endpoints
# -------------------------------------------------------------------- #


def _team_dir(team_id: str) -> Path:
    # Import at call-time so patches work in tests.
    import see_agent.config as _cfg

    return _cfg.TEAMS_DIR / team_id


@router.post("/{team_id}/message")
async def owner_send_message(
    team_id: str, body: OwnerMessageRequest, request: Request,
) -> StatusResponse:
    """Send a message from the owner to an agent."""
    from see_agent.team.bus import BusMessage
    from see_agent.team.definition import TeamDefinition

    try:
        TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    msg = BusMessage(sender="owner", recipient=body.to, content=body.content)

    # Inject into live bus if team is running.
    manager = request.app.state.team_managers.get(team_id)
    if manager is not None:
        manager._bus.send(msg)
    else:
        # Persist to audit log directly.
        log_path = _team_dir(team_id) / "messages.jsonl"
        log_path.parent.mkdir(parents=True, exist_ok=True)
        with open(log_path, "a", encoding="utf-8") as fh:
            fh.write(
                json.dumps(
                    {
                        "sender": msg.sender,
                        "recipient": msg.recipient,
                        "content": msg.content,
                        "ts": msg.ts,
                    },
                    ensure_ascii=False,
                )
                + "\n"
            )

    return StatusResponse(status="sent")


@router.get("/{team_id}/messages")
async def owner_get_messages(
    team_id: str,
    limit: int = Query(default=50, ge=1, le=500),
) -> list[TeamMessage]:
    """Get messages where sender or recipient is 'owner'."""
    from see_agent.team.definition import TeamDefinition

    try:
        TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    log_path = _team_dir(team_id) / "messages.jsonl"
    if not log_path.exists():
        return []

    results: list[TeamMessage] = []
    for line in log_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        entry = json.loads(line)
        if entry.get("sender") == "owner" or entry.get("recipient") == "owner":
            results.append(TeamMessage(**entry))

    return results[-limit:]


@router.get("/{team_id}/unread")
async def owner_unread_count(team_id: str) -> UnreadResponse:
    """Count unread messages sent to 'owner'."""
    from see_agent.team.definition import TeamDefinition

    try:
        TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    td = _team_dir(team_id)
    log_path = td / "messages.jsonl"
    if not log_path.exists():
        return UnreadResponse(unread=0)

    # Read last_read_ts from owner_state.json.
    state_path = td / "owner_state.json"
    last_read_ts = ""
    if state_path.exists():
        state = json.loads(state_path.read_text(encoding="utf-8"))
        last_read_ts = state.get("last_read_ts", "")

    count = 0
    for line in log_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        entry = json.loads(line)
        if entry.get("recipient") == "owner" and entry.get("ts", "") > last_read_ts:
            count += 1

    return UnreadResponse(unread=count)


@router.post("/{team_id}/mark_read")
async def owner_mark_read(team_id: str) -> MarkReadResponse:
    """Mark all owner messages as read."""
    from datetime import datetime, timezone

    from see_agent.team.definition import TeamDefinition

    try:
        TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    td = _team_dir(team_id)
    td.mkdir(parents=True, exist_ok=True)
    state_path = td / "owner_state.json"
    now = datetime.now(timezone.utc).isoformat()
    state_path.write_text(
        json.dumps({"last_read_ts": now}, ensure_ascii=False),
        encoding="utf-8",
    )
    return MarkReadResponse(last_read_ts=now)


# -------------------------------------------------------------------- #
# Logs & agent status
# -------------------------------------------------------------------- #


@router.get("/{team_id}/logs")
async def get_team_logs(
    team_id: str,
    limit: int = Query(default=100, ge=1, le=1000),
) -> list[TeamLogEntry]:
    """Return messages.jsonl content for a team."""
    from see_agent.team.definition import TeamDefinition

    try:
        TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    log_path = _team_dir(team_id) / "messages.jsonl"
    if not log_path.exists():
        return []

    results: list[TeamLogEntry] = []
    for line in log_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        results.append(TeamLogEntry(**json.loads(line)))

    return results[-limit:]


@router.get("/{team_id}/agents/{agent_id}/status")
async def get_agent_status(
    team_id: str, agent_id: str, request: Request,
) -> AgentStatusResponse:
    """Get an agent's session status within a team."""
    from see_agent.team.definition import TeamDefinition

    try:
        team = TeamDefinition.load(team_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Team not found")

    if agent_id not in team.members:
        raise HTTPException(
            status_code=404, detail="Agent not in team",
        )

    manager = request.app.state.team_managers.get(team_id)
    running = manager is not None
    return AgentStatusResponse(
        agent_id=agent_id,
        team_id=team_id,
        team_running=running,
    )
