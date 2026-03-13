"""API routes — tools listing."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel

from see_agent.server.schemas import ToolInfo

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["tools"])


@router.get("/tools")
async def list_tools(request: Request) -> list[ToolInfo]:
    """List all built-in tools (name, description, parameters)."""
    from see_agent.hand.tools import create_registry

    registry = create_registry()
    results: list[ToolInfo] = []
    for name, tool in registry._tools.items():
        results.append(ToolInfo(
            name=name,
            description=tool.description,
            parameters=tool.parameters,
        ))
    return results


@router.get("/agents/{agent_id}/tools")
async def get_agent_tools(agent_id: str) -> dict[str, Any]:
    """Return tool list with per-agent disabled status.

    Reads the actual tool manifest written by the agent worker
    (tools.json), falling back to create_registry() if not available.
    """
    import json as _json

    from see_agent.agent.definition import AgentDefinition
    from see_agent.config import AGENTS_DIR

    try:
        defn = AgentDefinition.load(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    disabled: list[str] = defn.tools.get("disabled", [])

    # Prefer actual tool manifest from worker.
    manifest_path = AGENTS_DIR / agent_id / "tools.json"
    if manifest_path.exists():
        manifest = _json.loads(manifest_path.read_text())
        tools = [
            {**t, "disabled": t["name"] in disabled}
            for t in manifest
        ]
    else:
        # Fallback: full registry (may include tools not used by worker).
        from see_agent.hand.tools import create_registry

        registry = create_registry()
        tools = [
            {
                "name": name,
                "description": tool.description,
                "disabled": name in disabled,
            }
            for name, tool in registry._tools.items()
        ]

    return {"tools": tools, "disabled": disabled}


class UpdateAgentToolsRequest(BaseModel):
    disabled: list[str]


@router.put("/agents/{agent_id}/tools")
async def update_agent_tools(
    agent_id: str, body: UpdateAgentToolsRequest,
) -> dict[str, Any]:
    """Update the agent's tools.disabled list."""
    from see_agent.agent.definition import AgentDefinition

    try:
        defn = AgentDefinition.load(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    defn.tools["disabled"] = body.disabled
    defn.save()
    return {"status": "ok", "disabled": body.disabled}
