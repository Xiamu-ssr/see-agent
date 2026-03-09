"""Agent management API routes."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/agents", tags=["agents"])


class UpdateAgentRequest(BaseModel):
    name: str | None = None
    role: str | None = None
    config_overrides: dict[str, Any] | None = None
    tools_config: dict[str, Any] | None = None
    skills_config: dict[str, Any] | None = None
    mcp_config: dict[str, Any] | None = None


@router.put("/{agent_id}")
async def update_agent(
    agent_id: str, body: UpdateAgentRequest,
) -> dict[str, Any]:
    """Update an existing agent definition."""
    from see_agent.agent.definition import AgentDefinition

    try:
        defn = AgentDefinition.load(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    if body.name is not None:
        defn.name = body.name
    if body.role is not None:
        defn.role = body.role
    if body.config_overrides is not None:
        defn.config_overrides = body.config_overrides
    if body.tools_config is not None:
        defn.tools_config = body.tools_config
    if body.skills_config is not None:
        defn.skills_config = body.skills_config
    if body.mcp_config is not None:
        defn.mcp_config = body.mcp_config

    defn.save()
    return {
        "id": defn.id,
        "name": defn.name,
        "role": defn.role,
    }
