"""Unified Message type for all agent communication.

v4: Simplified — no ``source`` field, only ``sender``.
``priority`` uses ``collect`` (not ``normal``) or ``steer``.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from datetime import datetime, timezone


@dataclass
class Message:
    """A message delivered to an agent.

    Attributes:
        sender: Sender identifier (user ID, agent ID, or "system").
        content: The message body.
        priority: "collect" (next loop batch) or "steer" (inject immediately).
        metadata: Arbitrary metadata (team_id, task_id, etc.).
        timestamp: ISO-8601 timestamp.
    """

    sender: str
    content: str
    priority: str = "collect"
    metadata: dict[str, str] = field(default_factory=dict)
    timestamp: str = field(
        default_factory=lambda: datetime.now(timezone.utc).isoformat(),
    )

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        return json.dumps(
            {
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
            sender=data["sender"],
            content=data["content"],
            priority=data.get("priority", "collect"),
            metadata=data.get("metadata", {}),
            timestamp=data.get("timestamp", ""),
        )

    def format_prefix(self) -> str:
        """Format a display prefix, e.g. ``[user]`` or ``[alice]``."""
        return f"[{self.sender}]"

    @property
    def is_steer(self) -> bool:
        """Whether this message should be injected into the current turn."""
        return self.priority == "steer"

    @property
    def is_shutdown(self) -> bool:
        """Whether this is a shutdown signal."""
        return self.sender == "system" and self.content == "shutdown"
