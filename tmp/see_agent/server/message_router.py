"""MessageRouter — central message dispatcher.

v4: Simplified — no ``source`` classification. Messages carry only
``sender`` and ``priority`` (collect/steer).
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING

from see_agent.ipc.message import Message

if TYPE_CHECKING:
    from see_agent.server.supervisor import AgentSupervisor

logger = logging.getLogger(__name__)


class MessageRouter:
    """Route messages between users and agents."""

    def __init__(self, supervisor: AgentSupervisor) -> None:
        self._supervisor = supervisor

    def on_user_message(
        self,
        agent_id: str,
        content: str,
        *,
        priority: str = "collect",
        sender: str = "user",
    ) -> None:
        """Handle a message from the user to an agent."""
        msg = Message(
            sender=sender,
            content=content,
            priority=priority,
        )
        self._supervisor.send_to(agent_id, msg)
        logger.info("User message → %s: %s", agent_id, content[:80])

    def on_agent_message(
        self,
        caller_id: str,
        target_id: str,
        content: str,
        *,
        team_id: str | None = None,
    ) -> None:
        """Handle a message from one agent to another."""
        msg = Message(
            sender=caller_id,
            content=content,
            metadata={"team_id": team_id} if team_id else {},
        )
        self._supervisor.send_to(target_id, msg)
        logger.info("Agent message %s → %s", caller_id, target_id)

    def on_task_notification(
        self,
        agent_id: str,
        task_title: str,
        *,
        task_id: str = "",
        team_id: str | None = None,
    ) -> None:
        """Notify an agent about a task assignment or update."""
        msg = Message(
            sender="system",
            content=f"Task assigned: {task_title}",
            metadata={
                k: v for k, v in [
                    ("task_id", task_id),
                    ("team_id", team_id or ""),
                ] if v
            },
        )
        self._supervisor.send_to(agent_id, msg)
