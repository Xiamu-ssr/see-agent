"""Pydantic data models for the see-agent HTTP / WebSocket API."""

from __future__ import annotations

from pydantic import BaseModel


class ChatRequest(BaseModel):
    """Request body for POST /api/chat."""

    task: str


class ChatResponse(BaseModel):
    """Response body for POST /api/chat."""

    task_id: str
    status: str = "running"


class TaskStatus(BaseModel):
    """Represents the current status of a running or completed task."""

    task_id: str
    status: str
    summary: str | None = None
    steps: int = 0
    error: str | None = None


class StepMessage(BaseModel):
    """A single agent step pushed over WebSocket."""

    step: int
    max_steps: int
    thought: str
    tool_name: str
    tool_args: dict
    tool_result: str
    screenshot_path: str
