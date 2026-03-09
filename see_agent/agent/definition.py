"""Agent definition — data model for agent identity and configuration.

Each agent lives in ``~/.see-agent/agents/{id}/`` with ``agent.json`` and
an optional ``SOUL.md`` personality file.
"""

from __future__ import annotations

import json
import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from see_agent.config import AGENTS_DIR, load_agent_config

logger = logging.getLogger(__name__)


@dataclass
class AgentDefinition:
    """Serialisable definition of a single agent."""

    id: str
    name: str
    role: str = "general assistant"
    config_overrides: dict[str, Any] = field(default_factory=dict)
    tools_config: dict[str, Any] = field(default_factory=dict)
    skills_config: dict[str, Any] = field(default_factory=dict)
    mcp_config: dict[str, Any] = field(default_factory=dict)
    soul_path: Path | None = None

    # ------------------------------------------------------------------ #
    # Persistence
    # ------------------------------------------------------------------ #

    def save(self) -> None:
        """Write ``agent.json`` (and optional SOUL.md) to disk."""
        agent_dir = AGENTS_DIR / self.id
        agent_dir.mkdir(parents=True, exist_ok=True)

        data: dict[str, Any] = {
            "id": self.id,
            "name": self.name,
            "role": self.role,
        }
        if self.config_overrides:
            data["config_overrides"] = self.config_overrides
        if self.tools_config:
            data["tools_config"] = self.tools_config
        if self.skills_config:
            data["skills_config"] = self.skills_config
        if self.mcp_config:
            data["mcp_config"] = self.mcp_config
        if self.soul_path is not None:
            data["soul_path"] = str(self.soul_path)

        (agent_dir / "agent.json").write_text(
            json.dumps(data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    @staticmethod
    def load(agent_id: str) -> AgentDefinition:
        """Load an agent definition from ``AGENTS_DIR/{agent_id}/agent.json``.

        Raises ``FileNotFoundError`` when the agent does not exist.
        """
        agent_json = AGENTS_DIR / agent_id / "agent.json"
        if not agent_json.exists():
            raise FileNotFoundError(
                f"Agent not found: {agent_id}"
            )
        data = json.loads(agent_json.read_text(encoding="utf-8"))
        soul_path_raw = data.get("soul_path")
        return AgentDefinition(
            id=data["id"],
            name=data.get("name", agent_id),
            role=data.get("role", "general assistant"),
            config_overrides=data.get("config_overrides", {}),
            tools_config=data.get("tools_config", {}),
            skills_config=data.get("skills_config", {}),
            mcp_config=data.get("mcp_config", {}),
            soul_path=Path(soul_path_raw) if soul_path_raw else None,
        )

    @staticmethod
    def create(agent_id: str, **kwargs: Any) -> AgentDefinition:
        """Create and persist a new agent definition."""
        defn = AgentDefinition(id=agent_id, **kwargs)
        defn.save()
        return defn

    @staticmethod
    def list_all() -> list[AgentDefinition]:
        """Return all agent definitions found on disk."""
        if not AGENTS_DIR.exists():
            return []
        results: list[AgentDefinition] = []
        for d in sorted(AGENTS_DIR.iterdir()):
            agent_json = d / "agent.json"
            if agent_json.exists():
                try:
                    results.append(AgentDefinition.load(d.name))
                except Exception:
                    logger.warning(
                        "Failed to load agent %s", d.name,
                    )
        return results

    # ------------------------------------------------------------------ #
    # Config helpers
    # ------------------------------------------------------------------ #

    def get_merged_config(self) -> dict[str, Any]:
        """Return fully merged config: global → agent overrides → env."""
        return load_agent_config(self.id)
