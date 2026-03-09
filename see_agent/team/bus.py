"""TeamBus — inter-agent message passing with JSONL audit log."""

from __future__ import annotations

import asyncio
import json
import logging
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

logger = logging.getLogger(__name__)


@dataclass
class BusMessage:
    """A single message on the team bus."""

    sender: str
    recipient: str  # agent_id or "__all__"
    content: str
    ts: str = ""

    def __post_init__(self) -> None:
        if not self.ts:
            self.ts = datetime.now(timezone.utc).isoformat()


class TeamBus:
    """In-process message bus for team collaboration.

    Each registered agent gets an :class:`asyncio.Queue`.
    All messages are also appended to ``messages.jsonl`` for auditing.
    """

    def __init__(self, team_dir: Path) -> None:
        self._team_dir = team_dir
        self._queues: dict[str, asyncio.Queue[BusMessage]] = {}
        self._log_path = team_dir / "messages.jsonl"
        team_dir.mkdir(parents=True, exist_ok=True)

    def register(self, agent_id: str) -> None:
        """Create a queue for *agent_id*."""
        if agent_id not in self._queues:
            self._queues[agent_id] = asyncio.Queue()

    def send(self, msg: BusMessage) -> None:
        """Send *msg* to the recipient's queue and log it."""
        self._log(msg)
        if msg.recipient == "__all__":
            self.broadcast(msg.sender, msg.content)
            return
        q = self._queues.get(msg.recipient)
        if q is not None:
            q.put_nowait(msg)
        else:
            logger.warning(
                "Bus: no queue for recipient '%s'", msg.recipient,
            )

    def broadcast(self, sender: str, content: str) -> None:
        """Send *content* to all agents except *sender*."""
        ts = datetime.now(timezone.utc).isoformat()
        for agent_id, q in self._queues.items():
            if agent_id != sender:
                q.put_nowait(
                    BusMessage(
                        sender=sender,
                        recipient=agent_id,
                        content=content,
                        ts=ts,
                    )
                )

    def drain(self, agent_id: str) -> list[BusMessage]:
        """Non-blocking drain of all pending messages for *agent_id*."""
        q = self._queues.get(agent_id)
        if q is None:
            return []
        messages: list[BusMessage] = []
        while True:
            try:
                messages.append(q.get_nowait())
            except asyncio.QueueEmpty:
                break
        return messages

    def get_queue(self, agent_id: str) -> asyncio.Queue[BusMessage]:
        """Return the raw queue for *agent_id*."""
        return self._queues[agent_id]

    def has_prior_message(self, from_: str, to: str) -> bool:
        """Check messages.jsonl for any message from *from_* to *to*."""
        if not self._log_path.exists():
            return False
        for line in self._log_path.read_text().splitlines():
            if not line.strip():
                continue
            entry = json.loads(line)
            if entry.get("sender") == from_ and entry.get("recipient") == to:
                return True
        return False

    def _log(self, msg: BusMessage) -> None:
        try:
            with open(self._log_path, "a", encoding="utf-8") as fh:
                fh.write(
                    json.dumps(
                        {
                            "sender": msg.sender,
                            "recipient": msg.recipient,
                            "content": msg.content,
                            "ts": msg.ts,
                        },
                        ensure_ascii=False,
                    )
                    + "\n"
                )
        except Exception:
            logger.exception("Bus log write failed")
