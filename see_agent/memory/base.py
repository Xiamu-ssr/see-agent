"""Abstract base class for memory backends."""

from __future__ import annotations

from abc import ABC, abstractmethod


class BaseMemory(ABC):
    """Interface that every memory backend must implement."""

    @abstractmethod
    def search(self, query: str, limit: int = 5) -> list[dict[str, str]]:
        """Search for relevant memories given a *query*.

        Returns:
            A list of dicts with ``file`` and ``snippet`` keys,
            most relevant first.
        """
        ...

    @abstractmethod
    def write(self, file: str, content: str) -> None:
        """Append *content* to a memory file.

        Parameters:
            file: Target filename (``MEMORY.md`` or ``YYYY-MM-DD.md``).
            content: Markdown text to append.
        """
        ...
