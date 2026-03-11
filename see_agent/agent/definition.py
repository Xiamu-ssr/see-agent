"""Agent definition — data model for agent identity and configuration.

Each agent lives in ``~/.see-agent/agents/{id}/`` with ``agent.json``,
a ``workspace/`` directory for prompt injection files, and ``memory/``
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
    """Serialisable definition of a single agent."""

    id: str
    name: str
    role: str = "general assistant"
    config_overrides: dict[str, Any] = field(default_factory=dict)
    tools_config: dict[str, Any] = field(default_factory=dict)
    skills_config: dict[str, Any] = field(default_factory=dict)
    mcp_config: dict[str, Any] = field(default_factory=dict)
    sandbox: dict[str, Any] = field(default_factory=dict)
    soul_path: Path | None = None

    # ------------------------------------------------------------------ #
    # Persistence
    # ------------------------------------------------------------------ #

    def save_to(self, base_dir: Path) -> None:
        """Write ``agent.json`` to *base_dir/{self.id}/*."""
        agent_dir = base_dir / self.id
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
        if self.sandbox:
            data["sandbox"] = self.sandbox
        if self.soul_path is not None:
            data["soul_path"] = str(self.soul_path)

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
        """
        agent_json = base_dir / agent_id / "agent.json"
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
            sandbox=data.get("sandbox", {}),
            soul_path=Path(soul_path_raw) if soul_path_raw else None,
        )

    @staticmethod
    def load(agent_id: str) -> AgentDefinition:
        """Load an agent definition from ``AGENTS_DIR/{agent_id}/agent.json``."""
        return AgentDefinition.load_from(AGENTS_DIR, agent_id)

    @staticmethod
    def create(agent_id: str, **kwargs: Any) -> AgentDefinition:
        """Create and persist a new agent definition.

        Also sets up the agent directory with:
        - ``workspace/`` subdirectory with template files
        - ``memory/`` subdirectory
        """
        defn = AgentDefinition(id=agent_id, **kwargs)
        defn.save()

        agent_dir = AGENTS_DIR / agent_id
        (agent_dir / "memory").mkdir(exist_ok=True)

        # Create workspace directory with template files.
        ws_dir = agent_dir / "workspace"
        ws_dir.mkdir(exist_ok=True)

        for template_name in (
            "AGENTS.md", "SOUL.md", "IDENTITY.md", "TOOLS.md", "USER.md",
        ):
            target = ws_dir / template_name
            source = _TEMPLATE_DIR / template_name
            if not target.exists() and source.exists():
                shutil.copy2(source, target)

        # Backward compat: also copy AGENTS.md/SOUL.md to agent root
        # (v3.2 code reads from agent_dir directly).
        for template_name in ("AGENTS.md", "SOUL.md"):
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
    def list_all_global() -> list[tuple[AgentDefinition, str | None]]:
        """Return all agents with team membership info.

        Each entry is ``(definition, team_id_or_None)``.
        Agents are only stored in ``AGENTS_DIR``; team membership is
        determined by cross-referencing ``team.json`` files.
        """
        from see_agent.team.definition import TeamDefinition

        # Build agent→team mapping from team definitions.
        agent_team_map: dict[str, str] = {}
        for team in TeamDefinition.list_all():
            for member_id in team.members:
                agent_team_map[member_id] = team.id

        agents = AgentDefinition.list_all()
        return [
            (defn, agent_team_map.get(defn.id))
            for defn in agents
        ]

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
            if self.id in team.members:
                return team.id
        return None

    # ------------------------------------------------------------------ #
    # Config helpers
    # ------------------------------------------------------------------ #

    def get_merged_config(self) -> dict[str, Any]:
        """Return fully merged config: global → agent overrides → env."""
        return load_agent_config(self.id)
