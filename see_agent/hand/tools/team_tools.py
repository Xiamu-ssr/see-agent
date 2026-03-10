"""Team collaboration tools — registered when running in team mode."""

from __future__ import annotations

from typing import Any

from see_agent.hand.tool import Tool

# -------------------------------------------------------------------- #
# Messaging
# -------------------------------------------------------------------- #


class SendMessageTool(Tool):
    """Send a message to a teammate."""

    def __init__(
        self, bus: Any, sender_id: str,
        leader_id: str | None = None,
    ) -> None:
        self._bus = bus
        self._sender_id = sender_id
        self._leader_id = leader_id

    @property
    def name(self) -> str:
        return "send_message"

    @property
    def description(self) -> str:
        return (
            "Send a message to a teammate, 'owner', or broadcast to all."
        )

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": (
                        "Recipient agent ID, 'owner', or '__all__' to broadcast."
                    ),
                },
                "content": {
                    "type": "string",
                    "description": "Message content.",
                },
            },
            "required": ["to", "content"],
        }

    async def execute(self, **kwargs: Any) -> str:
        from see_agent.team.bus import BusMessage

        to = kwargs["to"]
        content = kwargs["content"]

        # Permission check: non-leader agents need prior owner message to reply.
        if (
            to == "owner"
            and self._sender_id != self._leader_id
            and not self._bus.has_prior_message("owner", self._sender_id)
        ):
            return (
                "Permission denied: only the leader or agents who have "
                "received a message from the owner can message the owner."
            )

        self._bus.send(
            BusMessage(
                sender=self._sender_id,
                recipient=to,
                content=content,
            )
        )
        return f"Message sent to {to}."


# -------------------------------------------------------------------- #
# Task board tools
# -------------------------------------------------------------------- #


class ListTasksTool(Tool):
    """List tasks on the board."""

    def __init__(self, board: Any) -> None:
        self._board = board

    @property
    def name(self) -> str:
        return "list_tasks"

    @property
    def description(self) -> str:
        return "List tasks on the team task board."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter by status (optional).",
                },
            },
        }

    async def execute(self, **kwargs: Any) -> str:
        status = kwargs.get("status")
        tasks = self._board.list_tasks(status=status)
        if not tasks:
            return "No tasks found."
        lines = []
        for t in tasks:
            assignee = t.assigned_to or "unassigned"
            lines.append(
                f"- [{t.id}] {t.title} ({t.status}, {assignee})"
            )
        return "\n".join(lines)


class CreateTaskTool(Tool):
    """Create a new task."""

    def __init__(
        self, board: Any, creator_id: str,
    ) -> None:
        self._board = board
        self._creator_id = creator_id

    @property
    def name(self) -> str:
        return "create_task"

    @property
    def description(self) -> str:
        return "Create a new task on the task board."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Task title.",
                },
                "description": {
                    "type": "string",
                    "description": "Task description.",
                },
            },
            "required": ["title"],
        }

    async def execute(self, **kwargs: Any) -> str:
        task = self._board.create_task(
            title=kwargs["title"],
            description=kwargs.get("description", ""),
            created_by=self._creator_id,
        )
        return f"Task created: {task.id} — {task.title}"


class ClaimTaskTool(Tool):
    """Claim a task."""

    def __init__(
        self, board: Any, agent_id: str,
    ) -> None:
        self._board = board
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "claim_task"

    @property
    def description(self) -> str:
        return "Claim a task from the task board."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "ID of the task to claim.",
                },
            },
            "required": ["task_id"],
        }

    async def execute(self, **kwargs: Any) -> str:
        task = self._board.claim_task(
            kwargs["task_id"], self._agent_id,
        )
        return f"Claimed task {task.id}: {task.title}"


class CompleteTaskTool(Tool):
    """Complete a task."""

    def __init__(
        self, board: Any, agent_id: str,
    ) -> None:
        self._board = board
        self._agent_id = agent_id

    @property
    def name(self) -> str:
        return "complete_task"

    @property
    def description(self) -> str:
        return "Mark a task as done with a result."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "ID of the task to complete.",
                },
                "result": {
                    "type": "string",
                    "description": "Result or summary of work done.",
                },
            },
            "required": ["task_id"],
        }

    async def execute(self, **kwargs: Any) -> str:
        task = self._board.complete_task(
            kwargs["task_id"],
            self._agent_id,
            result=kwargs.get("result", ""),
        )
        return f"Completed task {task.id}: {task.title}"


class UpdateTaskTool(Tool):
    """Update task status."""

    def __init__(self, board: Any) -> None:
        self._board = board

    @property
    def name(self) -> str:
        return "update_task"

    @property
    def description(self) -> str:
        return "Update the status of a task."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "ID of the task to update.",
                },
                "status": {
                    "type": "string",
                    "description": "New status.",
                },
            },
            "required": ["task_id", "status"],
        }

    async def execute(self, **kwargs: Any) -> str:
        task = self._board.update_task(
            kwargs["task_id"], status=kwargs["status"],
        )
        return f"Updated task {task.id} status to {task.status}"


class AssignTaskTool(Tool):
    """Assign a task to an agent."""

    def __init__(self, board: Any) -> None:
        self._board = board

    @property
    def name(self) -> str:
        return "assign_task"

    @property
    def description(self) -> str:
        return "Assign a task to a specific agent."

    @property
    def parameters(self) -> dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "ID of the task.",
                },
                "agent_id": {
                    "type": "string",
                    "description": "Agent ID to assign to.",
                },
            },
            "required": ["task_id", "agent_id"],
        }

    async def execute(self, **kwargs: Any) -> str:
        task = self._board.assign_task(
            kwargs["task_id"], kwargs["agent_id"],
        )
        return f"Assigned task {task.id} to {task.assigned_to}"
