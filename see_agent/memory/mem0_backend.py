"""Mem0-based memory backend.

Requires the optional ``mem0ai`` package::

    pip install see-agent[memory]
"""

from __future__ import annotations

import logging
from typing import Any

from see_agent.memory.base import BaseMemory

logger = logging.getLogger(__name__)


def _build_mem0_config(config: dict[str, Any]) -> dict[str, Any] | None:
    """Build a mem0 ``Memory.from_config`` dict from our flat config.

    Returns ``None`` if all fields are empty (use mem0 defaults).
    """
    from pathlib import Path

    mem0_cfg: dict[str, Any] = {}

    # LLM provider
    llm_base_url = config.get("llm_base_url", "")
    llm_api_key = config.get("llm_api_key", "")
    llm_model = config.get("llm_model", "")
    if llm_model:
        llm_section: dict[str, Any] = {"provider": "openai", "config": {"model": llm_model}}
        if llm_base_url:
            llm_section["config"]["openai_base_url"] = llm_base_url
        if llm_api_key:
            llm_section["config"]["api_key"] = llm_api_key
        mem0_cfg["llm"] = llm_section

    # Embedding model
    embedding_model = config.get("embedding_model", "")
    if embedding_model:
        mem0_cfg["embedder"] = {
            "provider": "openai",
            "config": {"model": embedding_model},
        }
        if llm_base_url:
            mem0_cfg["embedder"]["config"]["openai_base_url"] = llm_base_url
        if llm_api_key:
            mem0_cfg["embedder"]["config"]["api_key"] = llm_api_key

    # Vector store (qdrant local)
    storage_path = config.get("storage_path", "")
    if storage_path:
        expanded = str(Path(storage_path).expanduser())
        mem0_cfg["vector_store"] = {
            "provider": "qdrant",
            "config": {"path": expanded},
        }

    return mem0_cfg if mem0_cfg else None


class Mem0Memory(BaseMemory):
    """Memory backend powered by `mem0ai <https://github.com/mem0ai/mem0>`_.

    Parameters:
        config: Optional mem0 configuration dict from ``config.memory.mem0``.
            Supports keys: ``llm_base_url``, ``llm_api_key``, ``llm_model``,
            ``embedding_model``, ``storage_path``.
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

        mem0_config = _build_mem0_config(config) if config else None
        self._mem = Memory.from_config(mem0_config) if mem0_config else Memory()
        self._user_id = user_id

    def search(self, query: str, limit: int = 5, agent_id: str | None = None) -> list[str]:
        """Search mem0 for relevant memories."""
        try:
            kwargs: dict[str, Any] = {"user_id": self._user_id, "limit": limit}
            if agent_id:
                kwargs["metadata"] = {"agent_id": agent_id}
            results = self._mem.search(query, **kwargs)
            if isinstance(results, dict) and "results" in results:
                results = results["results"]
            return [
                r.get("memory", r.get("text", str(r)))  # type: ignore[union-attr]
                for r in results
            ]
        except Exception:
            logger.exception("mem0 search failed")
            return []

    def add(self, messages: list[dict], session_id: str, agent_id: str | None = None) -> None:
        """Add conversation messages to mem0."""
        try:
            metadata: dict[str, str] = {"session_id": session_id}
            if agent_id:
                metadata["agent_id"] = agent_id
            self._mem.add(
                messages,
                user_id=self._user_id,
                metadata=metadata,
            )
        except Exception:
            logger.exception("mem0 add failed")
