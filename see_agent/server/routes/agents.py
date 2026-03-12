"""Agent management API routes."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel

from see_agent.server.schemas import (
    AgentCreateResponse,
    AgentDetail,
    AgentSummary,
    ChatMessage,
    SandboxAllowResponse,
    SandboxViolation,
    StatusResponse,
    WorkspaceFileContent,
    WorkspaceFileItem,
)

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/agents", tags=["agents"])


# -------------------------------------------------------------------- #
# Request models
# -------------------------------------------------------------------- #


class CreateAgentRequest(BaseModel):
    id: str | None = None
    soul: str | None = None
    llm: dict[str, Any] | None = None
    agent: dict[str, Any] | None = None
    screen: dict[str, Any] | None = None
    tools: dict[str, Any] | None = None
    skills: dict[str, Any] | None = None
    mcp: dict[str, Any] | None = None
    sandbox: dict[str, Any] | None = None


class UpdateAgentRequest(BaseModel):
    llm: dict[str, Any] | None = None
    agent: dict[str, Any] | None = None
    screen: dict[str, Any] | None = None
    tools: dict[str, Any] | None = None
    skills: dict[str, Any] | None = None
    mcp: dict[str, Any] | None = None
    sandbox: dict[str, Any] | None = None


# -------------------------------------------------------------------- #
# Routes — list / detail must be registered before /{agent_id}
# -------------------------------------------------------------------- #


@router.get("")
async def list_agents(request: Request) -> list[AgentSummary]:
    """List all agents with status."""
    from see_agent.agent.definition import AgentDefinition
    from see_agent.team.definition import TeamDefinition

    all_agents = AgentDefinition.list_all()
    supervisor = getattr(request.app.state, "supervisor", None)

    results: list[AgentSummary] = []
    for defn in all_agents:
        team_id = defn.get_team()
        team_name: str | None = None
        status = "idle"
        if supervisor and supervisor.is_running(defn.id):
            status = "busy"
        if team_id is not None:
            try:
                td = TeamDefinition.load(team_id)
                team_name = td.name
            except FileNotFoundError:
                team_name = team_id
        results.append(AgentSummary(
            id=defn.id,
            team_id=team_id,
            team_name=team_name,
            status=status,
        ))
    return results


@router.get("/{agent_id}")
async def get_agent(agent_id: str) -> AgentDetail:
    """Get detailed information about a single agent."""
    from see_agent.agent.definition import AgentDefinition
    from see_agent.team.definition import TeamDefinition

    try:
        defn, agent_dir = AgentDefinition.find(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    team_id = defn.get_team()
    team_name: str | None = None
    if team_id is not None:
        try:
            td = TeamDefinition.load(team_id)
            team_name = td.name
        except FileNotFoundError:
            team_name = team_id

    has_soul = (agent_dir / "SOUL.md").exists()

    return AgentDetail(
        id=defn.id,
        tools=defn.tools,
        skills=defn.skills,
        mcp=defn.mcp,
        sandbox=defn.sandbox,
        team_id=team_id,
        team_name=team_name,
        has_soul=has_soul,
        location=str(agent_dir),
    )


@router.post("")
async def create_agent(body: CreateAgentRequest) -> AgentCreateResponse:
    """Create a new agent definition."""
    import secrets
    import string

    from see_agent.agent.definition import AgentDefinition
    from see_agent.config import AGENTS_DIR

    agent_id = body.id or "".join(
        secrets.choice(string.ascii_lowercase + string.digits) for _ in range(6)
    )

    agent_json = AGENTS_DIR / agent_id / "agent.json"
    if agent_json.exists():
        raise HTTPException(status_code=409, detail="Agent already exists")

    kwargs: dict[str, Any] = {}
    for key in ("llm", "agent", "screen", "tools", "skills", "mcp", "sandbox"):
        value = getattr(body, key, None)
        if value is not None:
            kwargs[key] = value

    defn = AgentDefinition.create(agent_id, **kwargs)
    logger.info("Agent created: %s", defn.id)

    if body.soul:
        soul_path = AGENTS_DIR / agent_id / "SOUL.md"
        soul_path.write_text(body.soul, encoding="utf-8")

    return AgentCreateResponse(id=defn.id)


@router.put("/{agent_id}")
async def update_agent(
    agent_id: str, body: UpdateAgentRequest,
) -> AgentCreateResponse:
    """Update an existing agent definition."""
    from see_agent.agent.definition import AgentDefinition

    try:
        defn, agent_dir = AgentDefinition.find(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    for key in ("llm", "agent", "screen", "tools", "skills", "mcp", "sandbox"):
        value = getattr(body, key, None)
        if value is not None:
            setattr(defn, key, value)

    defn.save_to(agent_dir.parent)
    logger.info("Agent updated: %s", defn.id)
    return AgentCreateResponse(id=defn.id)


# -------------------------------------------------------------------- #
# Sandbox routes
# -------------------------------------------------------------------- #


class SandboxAllowRequest(BaseModel):
    path: str
    mode: str = "read"  # "read" | "write"


@router.get("/{agent_id}/sandbox/violations")
async def get_sandbox_violations(
    agent_id: str,
) -> list[SandboxViolation]:
    """Get recent sandbox deny records for an agent."""
    from see_agent.sandbox.collector import SandboxViolationCollector

    collector = SandboxViolationCollector()
    return [
        SandboxViolation(**v) for v in await collector.collect(agent_pid=0)
    ]


@router.post("/{agent_id}/sandbox/allow")
async def sandbox_allow(
    agent_id: str, body: SandboxAllowRequest,
) -> SandboxAllowResponse:
    """Add a path to the agent's sandbox allow list."""
    from see_agent.agent.definition import AgentDefinition

    try:
        defn, agent_dir = AgentDefinition.find(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    sandbox = defn.sandbox or {}
    if body.mode == "write":
        extra = sandbox.setdefault("extra_write", [])
    else:
        extra = sandbox.setdefault("extra_read", [])

    if body.path not in extra:
        extra.append(body.path)

    defn.sandbox = sandbox
    defn.save_to(agent_dir.parent)

    return SandboxAllowResponse(
        status="allowed",
        path=body.path,
        mode=body.mode,
    )


# -------------------------------------------------------------------- #
# Agent messaging & lifecycle (v3.5)
# -------------------------------------------------------------------- #


class SendMessageRequest(BaseModel):
    content: str
    priority: str = "normal"


class WorkspaceWriteRequest(BaseModel):
    content: str


@router.post("/{agent_id}/message")
async def send_agent_message(
    agent_id: str, body: SendMessageRequest, request: Request,
) -> StatusResponse:
    """Send a message to an agent."""
    from see_agent.agent.definition import AgentDefinition

    try:
        AgentDefinition.find(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    router_obj = getattr(request.app.state, "message_router", None)
    if router_obj is not None:
        router_obj.on_user_message(
            agent_id, body.content, priority=body.priority,
        )
    return StatusResponse(status="sent")


@router.get("/{agent_id}/chat")
async def get_agent_chat(agent_id: str) -> list[ChatMessage]:
    """Get chat history for an agent (single session)."""
    import json

    from see_agent.config import AGENTS_DIR

    messages_file = AGENTS_DIR / agent_id / "session" / "messages.jsonl"
    if not messages_file.exists():
        return []

    results: list[ChatMessage] = []
    for line in messages_file.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            entry = json.loads(line)
            results.append(ChatMessage(
                role=entry.get("role", "unknown"),
                content=entry.get("content"),
                timestamp=entry.get("timestamp"),
            ))
        except json.JSONDecodeError:
            continue

    return results[-100:]


@router.post("/{agent_id}/start")
async def start_agent(
    agent_id: str, request: Request,
) -> StatusResponse:
    """Start an agent subprocess."""
    from see_agent.agent.definition import AgentDefinition

    try:
        AgentDefinition.find(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    supervisor = getattr(request.app.state, "supervisor", None)
    if supervisor is not None:
        supervisor.start_agent(agent_id)
    return StatusResponse(status="started")


@router.post("/{agent_id}/stop")
async def stop_agent(
    agent_id: str, request: Request,
) -> StatusResponse:
    """Stop an agent subprocess."""
    supervisor = getattr(request.app.state, "supervisor", None)
    if supervisor is not None:
        supervisor.stop_agent(agent_id)
    return StatusResponse(status="stopped")


@router.get("/{agent_id}/workspace")
async def list_workspace_files(agent_id: str) -> list[WorkspaceFileItem]:
    """List md files in an agent's directory."""
    from see_agent.config import AGENTS_DIR

    agent_dir = AGENTS_DIR / agent_id
    if not agent_dir.is_dir():
        return []

    return [
        WorkspaceFileItem(name=f.name, size=f.stat().st_size)
        for f in sorted(agent_dir.iterdir())
        if f.is_file() and f.suffix == ".md"
    ]


@router.get("/{agent_id}/workspace/{filename}")
async def get_workspace_file(
    agent_id: str, filename: str,
) -> WorkspaceFileContent:
    """Read an agent file."""
    from see_agent.config import AGENTS_DIR

    fpath = AGENTS_DIR / agent_id / filename
    if not fpath.is_file():
        raise HTTPException(status_code=404, detail="File not found")

    content = fpath.read_text(encoding="utf-8")
    return WorkspaceFileContent(name=filename, content=content)


@router.put("/{agent_id}/workspace/{filename}")
async def update_workspace_file(
    agent_id: str, filename: str, body: WorkspaceWriteRequest,
) -> StatusResponse:
    """Write an agent file."""
    from see_agent.config import AGENTS_DIR

    agent_dir = AGENTS_DIR / agent_id
    if not agent_dir.is_dir():
        raise HTTPException(status_code=404, detail="Agent not found")

    fpath = agent_dir / filename
    fpath.write_text(body.content, encoding="utf-8")
    return StatusResponse(status="saved")
