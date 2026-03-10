"""API routes — tools listing."""

from __future__ import annotations

import logging

from fastapi import APIRouter, Request

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
