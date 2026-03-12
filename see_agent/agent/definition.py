"""Agent definition — data model for agent identity and configuration.

Each agent lives in ``~/.see-agent/agents/{id}/`` with ``agent.json``,
prompt injection files (IDENTITY.md, AGENTS.md, SOUL.md), and ``memory/``
for daily notes.
"""

from __future__ import annotations

import json
import logging
import shutil
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from see_agent.config import _TEMPLATE_DIR, AGENTS_DIR, load_agent_config

logger = logging.getLogger(__name__)


@dataclass
class AgentDefinition:
    """Serialisable definition of a single agent.

    ``agent.json`` is config-shaped (same structure as ``config.json``).
    Only the ``id`` field is agent-specific; all other keys are optional
    config overrides that deep-merge on top of the global config.
    """

    id: str
    llm: dict[str, Any] = field(default_factory=dict)
    agent: dict[str, Any] = field(default_factory=dict)
    screen: dict[str, Any] = field(default_factory=dict)
    tools: dict[str, Any] = field(default_factory=dict)
    skills: dict[str, Any] = field(default_factory=dict)
    mcp: dict[str, Any] = field(default_factory=dict)
    sandbox: dict[str, Any] = field(default_factory=dict)

    # ------------------------------------------------------------------ #
    # Persistence
    # ------------------------------------------------------------------ #

    def save_to(self, base_dir: Path) -> None:
        """Write ``agent.json`` to *base_dir/{self.id}/*."""
        agent_dir = base_dir / self.id
        agent_dir.mkdir(parents=True, exist_ok=True)

        data: dict[str, Any] = {"id": self.id}
        for key in ("llm", "agent", "screen", "tools", "skills", "mcp", "sandbox"):
            value = getattr(self, key)
            if value:
                data[key] = value

        (agent_dir / "agent.json").write_text(
            json.dumps(data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    def save(self) -> None:
        """Write ``agent.json`` to the global agents directory."""
        self.save_to(AGENTS_DIR)

    @staticmethod
    def load_from(base_dir: Path, agent_id: str) -> AgentDefinition:
        """Load an agent definition from *base_dir/{agent_id}/agent.json*.

        Raises ``FileNotFoundError`` when the agent does not exist.
        Also migrates legacy workspace/ files to agent root if present.
        """
        agent_dir = base_dir / agent_id
        agent_json = agent_dir / "agent.json"
        if not agent_json.exists():
            raise FileNotFoundError(
                f"Agent not found: {agent_id}"
            )

        # Migrate legacy workspace/ remnants.
        ws_dir = agent_dir / "workspace"
        if ws_dir.is_dir():
            for f in ws_dir.iterdir():
                target = agent_dir / f.name
                if not target.exists():
                    f.rename(target)
            # Remove empty workspace dir.
            try:
                ws_dir.rmdir()
            except OSError:
                pass

        data = json.loads(agent_json.read_text(encoding="utf-8"))
        return AgentDefinition(
            id=data["id"],
            llm=data.get("llm", {}),
            agent=data.get("agent", {}),
            screen=data.get("screen", {}),
            tools=data.get("tools", {}),
            skills=data.get("skills", {}),
            mcp=data.get("mcp", {}),
            sandbox=data.get("sandbox", {}),
        )

    @staticmethod
    def load(agent_id: str) -> AgentDefinition:
        """Load an agent definition from ``AGENTS_DIR/{agent_id}/agent.json``."""
        return AgentDefinition.load_from(AGENTS_DIR, agent_id)

    @staticmethod
    def create(agent_id: str, **kwargs: Any) -> AgentDefinition:
        """Create and persist a new agent definition.

        Also sets up the agent directory with:
        - Template files (IDENTITY.md, AGENTS.md, SOUL.md)
        - ``memory/`` subdirectory
        """
        defn = AgentDefinition(id=agent_id, **kwargs)
        defn.save()

        agent_dir = AGENTS_DIR / agent_id
        (agent_dir / "memory").mkdir(exist_ok=True)

        # Copy template files directly to agent_dir.
        for template_name in ("IDENTITY.md", "AGENTS.md", "SOUL.md"):
            target = agent_dir / template_name
            source = _TEMPLATE_DIR / template_name
            if not target.exists() and source.exists():
                shutil.copy2(source, target)

        return defn

    @staticmethod
    def list_all() -> list[AgentDefinition]:
        """Return all agent definitions found in ``AGENTS_DIR``."""
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

    @staticmethod
    def find(agent_id: str) -> tuple[AgentDefinition, Path]:
        """Find an agent by *agent_id* in the global agents directory.

        Returns ``(definition, agent_dir_path)``.
        Raises ``FileNotFoundError`` if not found.
        """
        agent_dir = AGENTS_DIR / agent_id
        if not (agent_dir / "agent.json").exists():
            raise FileNotFoundError(f"Agent not found: {agent_id}")
        defn = AgentDefinition.load_from(AGENTS_DIR, agent_id)
        return defn, agent_dir

    def get_team(self) -> str | None:
        """Return the team ID this agent belongs to, or None."""
        from see_agent.team.definition import TeamDefinition

        for team in TeamDefinition.list_all():
            if self.id in [m["id"] for m in team.members]:
                return team.id
        return None

    # ------------------------------------------------------------------ #
    # Config helpers
    # ------------------------------------------------------------------ #

    def get_merged_config(self) -> dict[str, Any]:
        """Return fully merged config: global → agent overrides → env."""
        return load_agent_config(self.id)
