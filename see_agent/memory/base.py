"""Abstract base class for memory backends."""

from __future__ import annotations

from abc import ABC, abstractmethod


class BaseMemory(ABC):
    """Interface that every memory backend must implement."""

    @abstractmethod
    def search(self, query: str, limit: int = 5, agent_id: str | None = None) -> list[str]:
        """Search for relevant memories given a *query*.

        Returns:
            A list of memory strings, most relevant first.
        """
        ...

    @abstractmethod
    def add(self, messages: list[dict], session_id: str, agent_id: str | None = None) -> None:
        """Persist conversation *messages* to memory.

        Parameters:
            messages: The conversation history (already stripped of base64).
            session_id: Identifier for the session these messages belong to.
            agent_id: Optional agent identifier for multi-agent scoping.
        """
        ...

    def clear(self, agent_id: str | None = None) -> None:
        """Clear stored memories.  Default is a no-op."""
