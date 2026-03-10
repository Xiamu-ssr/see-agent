"""Memory tools — allow the agent to search and write memories."""

from __future__ import annotations

import logging
from typing import Any

from see_agent.hand.tool import Tool, ToolResult
from see_agent.memory.base import BaseMemory

logger = logging.getLogger(__name__)


class MemorySearchTool(Tool):
    """Search the agent's memory for relevant information."""

    def __init__(self, backend: BaseMemory) -> None:
        self._backend = backend

    @property
    def name(self) -> str:
        return "memory_search"

    @property
    def description(self) -> str:
        return "Search memory for relevant past experiences and knowledge."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query to find relevant memories.",
                },
            },
            "required": ["query"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        query: str = kwargs["query"]
        results = self._backend.search(query, limit=5)
        if not results:
            return ToolResult(text="No relevant memories found.")
        lines = []
        for r in results:
            lines.append(f"[{r['file']}] {r['snippet']}")
        return ToolResult(text="\n---\n".join(lines))


class WriteMemoryTool(Tool):
    """Write a memory entry to a markdown file."""

    def __init__(self, backend: BaseMemory) -> None:
        self._backend = backend

    @property
    def name(self) -> str:
        return "memory_write"

    @property
    def description(self) -> str:
        return (
            "Write important information to memory for future reference. "
            "Use MEMORY.md for persistent notes or YYYY-MM-DD.md for daily logs."
        )

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Target file: MEMORY.md or YYYY-MM-DD.md (e.g. 2024-01-15.md).",
                },
                "content": {
                    "type": "string",
                    "description": "Markdown content to append to the file.",
                },
            },
            "required": ["file", "content"],
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        file: str = kwargs["file"]
        content: str = kwargs["content"]
        try:
            self._backend.write(file, content)
            return ToolResult(text=f"Written to {file}.")
        except ValueError as exc:
            return ToolResult(text=f"Error: {exc}")
