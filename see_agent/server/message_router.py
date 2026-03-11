"""MessageRouter — central message dispatcher (server as post office).

v3.5: All inter-agent and user-to-agent messages flow through this
router.  It determines message source type (user/leader/teammate),
constructs a :class:`Message`, and forwards it to the target agent
via the :class:`AgentSupervisor`.
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Any

from see_agent.ipc.message import Message

if TYPE_CHECKING:
    from see_agent.server.supervisor import AgentSupervisor

logger = logging.getLogger(__name__)


class MessageRouter:
    """Route messages between users and agents.

    Parameters:
        supervisor: The agent process supervisor.
    """

    def __init__(self, supervisor: AgentSupervisor) -> None:
        self._supervisor = supervisor

    def on_user_message(
        self,
        agent_id: str,
        content: str,
        *,
        priority: str = "normal",
        sender: str = "user",
    ) -> None:
        """Handle a message from the user to an agent.

        Constructs a Message with source="user" and forwards it.
        """
        msg = Message(
            source="user",
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
        """Handle a message from one agent to another.

        Determines whether the caller is the team leader or a teammate,
        then forwards the message with the appropriate source type.
        """
        source = self._classify_source(caller_id, target_id, team_id)
        msg = Message(
            source=source,
            sender=caller_id,
            content=content,
            metadata={"team_id": team_id} if team_id else {},
        )
        self._supervisor.send_to(target_id, msg)
        logger.info(
            "Agent message %s → %s (source=%s)", caller_id, target_id, source,
        )

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
            source="task",
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

    def _classify_source(
        self,
        caller_id: str,
        target_id: str,
        team_id: str | None,
    ) -> str:
        """Determine message source type based on team roles."""
        if not team_id:
            return "teammate"

        team = self._find_team(team_id)
        if team and team.leader == caller_id:
            return "leader"
        return "teammate"

    @staticmethod
    def _find_team(team_id: str) -> Any:
        """Load a team definition, returning None if not found."""
        try:
            from see_agent.team.definition import TeamDefinition

            return TeamDefinition.load(team_id)
        except FileNotFoundError:
            return None
