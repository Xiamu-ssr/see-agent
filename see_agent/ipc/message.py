"""Unified Message type for all agent communication.

v3.5: Replaces the scattered BusMessage / user_queue / task notification
channels with a single Message dataclass that flows through the system.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from datetime import datetime, timezone


@dataclass
class Message:
    """A message delivered to an agent.

    Attributes:
        source: Origin category — "user", "leader", "teammate", "task", "system".
        sender: Sender identifier (user ID, agent ID, or "system").
        content: The message body.
        priority: "normal" (collect) or "steer" (inject into current turn).
        metadata: Arbitrary metadata (team_id, task_id, etc.).
        timestamp: ISO-8601 timestamp.
    """

    source: str
    sender: str
    content: str
    priority: str = "normal"
    metadata: dict[str, str] = field(default_factory=dict)
    timestamp: str = field(
        default_factory=lambda: datetime.now(timezone.utc).isoformat(),
    )

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        return json.dumps(
            {
                "source": self.source,
                "sender": self.sender,
                "content": self.content,
                "priority": self.priority,
                "metadata": self.metadata,
                "timestamp": self.timestamp,
            },
            ensure_ascii=False,
        )

    @classmethod
    def from_json(cls, raw: str) -> Message:
        """Deserialize from a JSON string."""
        data = json.loads(raw)
        return cls(
            source=data["source"],
            sender=data["sender"],
            content=data["content"],
            priority=data.get("priority", "normal"),
            metadata=data.get("metadata", {}),
            timestamp=data.get("timestamp", ""),
        )

    def format_prefix(self) -> str:
        """Format a display prefix, e.g. ``[user lanxuan]``."""
        return f"[{self.source} {self.sender}]"

    @property
    def is_steer(self) -> bool:
        """Whether this message should be injected into the current turn."""
        return self.priority == "steer"

    @property
    def is_shutdown(self) -> bool:
        """Whether this is a shutdown signal."""
        return self.source == "system" and self.content == "shutdown"
