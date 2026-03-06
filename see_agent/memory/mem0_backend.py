"""Mem0-based memory backend.

Requires the optional ``mem0ai`` package::

    pip install see-agent[memory]
"""

from __future__ import annotations

import logging
from typing import Any

from see_agent.memory.base import BaseMemory

logger = logging.getLogger(__name__)


class Mem0Memory(BaseMemory):
    """Memory backend powered by `mem0ai <https://github.com/mem0ai/mem0>`_.

    Parameters:
        config: Optional mem0 configuration dict passed to ``Memory()``.
        user_id: User identifier for mem0 memory scoping.
    """

    def __init__(self, config: dict[str, Any] | None = None, user_id: str = "see-agent") -> None:
        try:
            from mem0 import Memory  # type: ignore[import-untyped]
        except ImportError as exc:
            raise ImportError(
                "mem0ai is required for the memory feature. "
                "Install it with: pip install see-agent[memory]"
            ) from exc

        self._mem = Memory.from_config(config) if config else Memory()
        self._user_id = user_id

    def search(self, query: str, limit: int = 5) -> list[str]:
        """Search mem0 for relevant memories."""
        try:
            results = self._mem.search(query, user_id=self._user_id, limit=limit)
            if isinstance(results, dict) and "results" in results:
                results = results["results"]
            return [
                r.get("memory", r.get("text", str(r)))
                for r in results
            ]
        except Exception:
            logger.exception("mem0 search failed")
            return []

    def add(self, messages: list[dict], session_id: str) -> None:
        """Add conversation messages to mem0."""
        try:
            self._mem.add(
                messages,
                user_id=self._user_id,
                metadata={"session_id": session_id},
            )
        except Exception:
            logger.exception("mem0 add failed")
