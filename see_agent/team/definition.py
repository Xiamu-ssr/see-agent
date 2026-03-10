"""TeamDefinition — data model for a team of agents."""

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
    members: list[str] = field(default_factory=list)
    leader: str | None = None
    owner: dict[str, str] | None = None
    overrides: dict[str, Any] | None = None
    seating: dict[str, int] = field(default_factory=dict)
    screen_mode: str = "serial"
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
            "seating": self.seating,
            "screen_mode": self.screen_mode,
            "status": self.status,
            "created_at": self.created_at,
            "owner": self.owner,
            "overrides": self.overrides,
        }
        (team_dir / "team.json").write_text(
            json.dumps(data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    @staticmethod
    def create(
        name: str,
        members: list[str],
        leader: str | None = None,
        owner: dict[str, str] | None = None,
        overrides: dict[str, Any] | None = None,
        seating: dict[str, int] | None = None,
    ) -> TeamDefinition:
        """Create and persist a new team."""
        team_id = secrets.token_hex(4)
        now = datetime.now(timezone.utc).isoformat()
        defn = TeamDefinition(
            id=team_id,
            name=name,
            members=members,
            leader=leader,
            owner=owner,
            overrides=overrides,
            seating=seating or {},
            created_at=now,
        )
        defn.save()
        return defn

    @staticmethod
    def load(team_id: str) -> TeamDefinition:
        """Load a team from disk.

        Raises ``FileNotFoundError`` if the team does not exist.
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
            owner=data.get("owner"),
            overrides=data.get("overrides"),
            seating=data.get("seating", {}),
            screen_mode=data.get("screen_mode", "serial"),
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
