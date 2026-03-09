"""Context engine interface for system prompt construction.

Provides a pluggable layer between agent configuration and prompt assembly.
The default :class:`LegacyContextEngine` delegates to
:func:`~see_agent.brain.prompts.build_system_prompt`.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from see_agent.skill.loader import SkillInfo


class BaseContextEngine(ABC):
    """Interface for building system prompts."""

    @abstractmethod
    def build_prompt(
        self,
        config: dict[str, Any],
        *,
        skills: list[SkillInfo] | None = None,
        memory_block: str = "",
        team_context: str = "",
    ) -> str:
        """Build a full system prompt from *config* and optional sections."""
        ...

    @property
    def owns_compaction(self) -> bool:
        """Whether this engine handles context compaction internally."""
        return False


class LegacyContextEngine(BaseContextEngine):
    """Default engine — delegates to :func:`build_system_prompt`."""

    def build_prompt(
        self,
        config: dict[str, Any],
        *,
        skills: list[SkillInfo] | None = None,
        memory_block: str = "",
        team_context: str = "",
    ) -> str:
        from see_agent.brain.prompts import build_system_prompt

        return build_system_prompt(
            config,
            skills=skills,
            memory_block=memory_block,
            team_context=team_context,
        )
