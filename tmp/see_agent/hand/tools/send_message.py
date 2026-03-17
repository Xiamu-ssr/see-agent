"""send_message — send a message to a teammate agent."""

from __future__ import annotations

import json
import logging
from pathlib import Path

from see_agent.hand.tool import Tool, ToolResult

logger = logging.getLogger(__name__)


class SendMessageTool(Tool):
    """Send a message to another agent in the same team.

    Writes to the target agent's inbox.jsonl so the message
    is picked up by their inbox drain loop.
    """

    @property
    def name(self) -> str:
        return "send_message"

    @property
    def description(self) -> str:
        return (
            "Send a message to a teammate agent. "
            "Use this to coordinate, delegate tasks, or share findings."
        )

    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Target agent ID (must be a teammate).",
                },
                "message": {
                    "type": "string",
                    "description": "Message content to send.",
                },
            },
            "required": ["to", "message"],
        }

    def __init__(
        self,
        sender_id: str,
        team_member_ids: list[str],
        agents_dir: Path,
    ) -> None:
        self._sender_id = sender_id
        self._team_member_ids = team_member_ids
        self._agents_dir = agents_dir

    async def execute(self, **kwargs: str) -> ToolResult:
        to = kwargs.get("to", "")
        message = kwargs.get("message", "")

        if not to or not message:
            return ToolResult(text="Error: 'to' and 'message' are required.")

        if to not in self._team_member_ids:
            return ToolResult(
                text=f"Error: '{to}' is not a teammate. "
                f"Valid targets: {', '.join(self._team_member_ids)}",
            )

        if to == self._sender_id:
            return ToolResult(text="Error: cannot send message to yourself.")

        # Write to target agent's inbox.
        inbox_path = self._agents_dir / to / "inbox.jsonl"
        if not inbox_path.parent.is_dir():
            return ToolResult(text=f"Error: agent '{to}' directory not found.")

        # Read current max msg_id.
        max_id = 0
        if inbox_path.exists():
            for line in inbox_path.read_text().splitlines():
                line = line.strip()
                if not line:
                    continue
                try:
                    data = json.loads(line)
                    max_id = max(max_id, data.get("msg_id", 0))
                except json.JSONDecodeError:
                    continue

        from datetime import datetime, timezone

        entry = {
            "msg_id": max_id + 1,
            "sender": self._sender_id,
            "content": message,
            "priority": "collect",
            "metadata": {},
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }

        with open(inbox_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")

        logger.info(
            "Agent %s sent message to %s: %s",
            self._sender_id, to, message[:80],
        )

        # Try to wake the target agent via SIGUSR1.
        self._signal_agent(to)

        return ToolResult(
            text=f"Message sent to {to} successfully.",
        )

    def _signal_agent(self, agent_id: str) -> None:
        """Best-effort SIGUSR1 to wake the target agent."""
        import signal

        pid_candidates = []
        try:
            import subprocess

            result = subprocess.run(
                ["pgrep", "-f", f"see_agent.agent.worker {agent_id}"],
                capture_output=True, text=True, timeout=2,
            )
            for line in result.stdout.strip().splitlines():
                pid_candidates.append(int(line.strip()))
        except Exception:
            pass

        import os

        for pid in pid_candidates:
            try:
                os.kill(pid, signal.SIGUSR1)
                logger.debug("Sent SIGUSR1 to pid %d for agent %s", pid, agent_id)
            except OSError:
                pass
