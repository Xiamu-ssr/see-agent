"""Pydantic response models — single source of truth for API contracts."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel

# -------------------------------------------------------------------- #
# Agents
# -------------------------------------------------------------------- #


class AgentSummary(BaseModel):
    id: str
    name: str
    role: str
    team_id: str | None = None
    team_name: str | None = None
    status: str = "idle"


class AgentDetail(AgentSummary):
    config_overrides: dict[str, Any] = {}
    tools_config: dict[str, Any] = {}
    skills_config: dict[str, Any] = {}
    mcp_config: dict[str, Any] = {}
    sandbox: dict[str, Any] = {}
    has_soul: bool = False
    location: str = ""


class AgentCreateResponse(BaseModel):
    id: str
    name: str
    role: str


# -------------------------------------------------------------------- #
# Teams
# -------------------------------------------------------------------- #


class TaskItem(BaseModel):
    id: str
    title: str
    status: str
    assigned_to: str | None = None


class TeamSummary(BaseModel):
    id: str
    name: str
    members: list[str] = []
    status: str = "created"


class TeamStatus(TeamSummary):
    leader: str | None = None
    tasks: list[TaskItem] = []


class TeamCreateResponse(BaseModel):
    id: str
    name: str
    status: str


class TeamUpdateResponse(BaseModel):
    id: str
    name: str
    members: list[str]
    status: str


class TeamRunResponse(BaseModel):
    team_id: str
    success: bool
    summary: str


class TeamMessage(BaseModel):
    sender: str
    recipient: str
    content: str
    ts: str


class UnreadResponse(BaseModel):
    unread: int


class MarkReadResponse(BaseModel):
    last_read_ts: str


class StatusResponse(BaseModel):
    status: str


class AgentStatusResponse(BaseModel):
    agent_id: str
    team_id: str
    team_running: bool


# -------------------------------------------------------------------- #
# Team logs (same shape as TeamMessage)
# -------------------------------------------------------------------- #


class TeamLogEntry(BaseModel):
    sender: str
    recipient: str
    content: str
    ts: str


# -------------------------------------------------------------------- #
# Dashboard / Skills / Logs / Health / Tools / MCP
# -------------------------------------------------------------------- #


class DashboardResponse(BaseModel):
    teams_count: int
    teams_by_status: dict[str, int]
    agents_in_team: int
    agents_idle: int
    total_tasks: int
    tasks_by_status: dict[str, int]


class SkillInfo(BaseModel):
    name: str
    description: str
    available: bool


class SkillInstallResponse(BaseModel):
    status: str
    name: str


class LogEntry(BaseModel):
    time: str
    level: str
    logger: str
    message: str


class HealthResponse(BaseModel):
    status: str
    version: str


class ToolInfo(BaseModel):
    name: str
    description: str
    parameters: dict[str, Any] = {}


class McpInstallResponse(BaseModel):
    status: str
    name: str
    config: dict[str, Any]


# -------------------------------------------------------------------- #
# v3.1 — Sandbox / Screen lease
# -------------------------------------------------------------------- #


class SandboxViolation(BaseModel):
    timestamp: str
    operation: str
    path: str


class SandboxAllowResponse(BaseModel):
    status: str
    path: str
    mode: str  # "read" | "write"


class ScreenLeaseStatus(BaseModel):
    holder: str | None = None
    started_at: str | None = None
    idle_seconds: int = 0
    queue_length: int = 0


# -------------------------------------------------------------------- #
# Agent workspace / chat / lifecycle (v3.5)
# -------------------------------------------------------------------- #


class WorkspaceFileItem(BaseModel):
    name: str
    size: int


class WorkspaceFileContent(BaseModel):
    name: str
    content: str


class ChatMessage(BaseModel):
    role: str
    content: str | None = None
    timestamp: str | None = None
