"""API routes — tools listing."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, Request

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["tools"])


@router.get("/tools")
async def list_tools(request: Request) -> list[dict[str, Any]]:
    """List all built-in tools (name, description, parameters)."""
    from see_agent.hand.tools import create_registry

    registry = create_registry()
    results: list[dict[str, Any]] = []
    for name, tool in registry._tools.items():
        results.append({
            "name": name,
            "description": tool.description,
            "parameters": tool.parameters,
        })
    return results
