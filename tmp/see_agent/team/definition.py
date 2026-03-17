"""TeamDefinition — data model for a team of agents.

v3.5: Team is a lightweight "room" — just a member list + leader + status.
Agent data lives in agents/{id}/, not under teams/.
"""

from __future__ import annotations

import json
import logging
import secrets
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any

from see_agent.config import TEAMS_DIR

logger = logging.getLogger(__name__)


@dataclass
class TeamDefinition:
    """Serialisable definition of an agent team."""

    id: str
    name: str
    members: list[dict[str, str]] = field(default_factory=list)
    leader: str | None = None
    status: str = "created"
    created_at: str = ""

    # ------------------------------------------------------------------ #
    # Persistence
    # ------------------------------------------------------------------ #

    def save(self) -> None:
        """Write ``team.json`` to disk."""
        team_dir = TEAMS_DIR / self.id
        team_dir.mkdir(parents=True, exist_ok=True)
        data: dict[str, Any] = {
            "id": self.id,
            "name": self.name,
            "members": self.members,
            "leader": self.leader,
            "status": self.status,
            "created_at": self.created_at,
        }
        (team_dir / "team.json").write_text(
            json.dumps(data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    @staticmethod
    def create(
        name: str,
        members: list[dict[str, str]],
        leader: str | None = None,
    ) -> TeamDefinition:
        """Create and persist a new team."""
        team_id = secrets.token_hex(4)
        now = datetime.now(timezone.utc).isoformat()
        defn = TeamDefinition(
            id=team_id,
            name=name,
            members=members,
            leader=leader,
            created_at=now,
        )
        defn.save()
        # Write team_id into each member's agent.json.
        defn._sync_member_team_ids()
        return defn

    def _sync_member_team_ids(self) -> None:
        """Write this team's ID into each member's agent.json."""
        from see_agent.config import AGENTS_DIR

        for m in self.members:
            agent_json = AGENTS_DIR / m["id"] / "agent.json"
            if not agent_json.exists():
                continue
            data = json.loads(agent_json.read_text(encoding="utf-8"))
            if data.get("team_id") != self.id:
                data["team_id"] = self.id
                data["team_role"] = m.get("role", "worker")
                agent_json.write_text(
                    json.dumps(data, indent=2, ensure_ascii=False),
                    encoding="utf-8",
                )
                logger.info(
                    "Set team_id=%s on agent %s", self.id, m["id"],
                )

    @staticmethod
    def load(team_id: str) -> TeamDefinition:
        """Load a team from disk.

        Raises ``FileNotFoundError`` if the team does not exist.
        Ignores deprecated fields (owner, overrides, seating) for
        backward compatibility.
        """
        team_json = TEAMS_DIR / team_id / "team.json"
        if not team_json.exists():
            raise FileNotFoundError(f"Team not found: {team_id}")
        data = json.loads(team_json.read_text(encoding="utf-8"))
        return TeamDefinition(
            id=data["id"],
            name=data.get("name", ""),
            members=data.get("members", []),
            leader=data.get("leader"),
            status=data.get("status", "created"),
            created_at=data.get("created_at", ""),
        )

    @staticmethod
    def list_all() -> list[TeamDefinition]:
        """Return all team definitions."""
        if not TEAMS_DIR.exists():
            return []
        results: list[TeamDefinition] = []
        for d in sorted(TEAMS_DIR.iterdir()):
            team_json = d / "team.json"
            if team_json.exists():
                try:
                    results.append(TeamDefinition.load(d.name))
                except Exception:
                    logger.warning("Failed to load team %s", d.name)
        return results
