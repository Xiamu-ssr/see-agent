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

    Derives the tool set from agent config (deterministic),
    no need to query the subprocess.
    """
    from see_agent.agent.definition import AgentDefinition

    try:
        defn = AgentDefinition.load(agent_id)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Agent not found")

    disabled: list[str] = defn.tools.get("disabled", [])

    # Base tools every agent has.
    base_tools = [
        {"name": "shell", "description": "执行终端命令。"},
        {"name": "wait", "description": "等待指定秒数。"},
        {"name": "finished", "description": "任务完成，必须调用此工具表示任务结束。"},
        {"name": "memory_search", "description": "搜索记忆中的相关信息。"},
        {"name": "memory_write", "description": "将重要信息写入记忆。"},
    ]

    # Team tool.
    team_id = defn.get_team()
    if team_id:
        base_tools.append(
            {"name": "send_message", "description": "给队友发消息。"},
        )

    tools = [
        {**t, "disabled": t["name"] in disabled}
        for t in base_tools
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
