"""API routes — tools listing."""

from __future__ import annotations

import logging
from typing import Any

from fastapi import APIRouter, Request

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api", tags=["tools"])


class _NullEye:
    """Minimal eye for enumerating tools without real screen capture."""

    async def capture(self):  # type: ignore[override]  # noqa: ANN201
        raise NotImplementedError


@router.get("/tools")
async def list_tools(request: Request) -> list[dict[str, Any]]:
    """List all built-in tools (name, description, parameters)."""
    from see_agent.hand.tools import create_registry

    registry = create_registry(_NullEye())  # type: ignore[arg-type]
    results: list[dict[str, Any]] = []
    for name, tool in registry._tools.items():
        results.append({
            "name": name,
            "description": tool.description,
            "parameters": tool.parameters,
        })
    return results
