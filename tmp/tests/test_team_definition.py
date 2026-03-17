"""Unit tests for TeamDefinition."""

from __future__ import annotations

from unittest.mock import patch

import pytest

from see_agent.team.definition import TeamDefinition


@pytest.fixture
def teams_dir(tmp_path):
    d = tmp_path / "teams"
    d.mkdir()
    with patch("see_agent.team.definition.TEAMS_DIR", d):
        yield d


class TestTeamDefinition:

    def test_create(self, teams_dir):
        team = TeamDefinition.create(
            "My Team",
            [{"id": "alice", "role": "leader"}, {"id": "bob", "role": "worker"}],
            leader="alice",
        )
        assert team.name == "My Team"
        assert team.members == [{"id": "alice", "role": "leader"}, {"id": "bob", "role": "worker"}]
        assert team.leader == "alice"
        assert team.status == "created"
        assert (teams_dir / team.id / "team.json").exists()

    def test_load(self, teams_dir):
        team = TeamDefinition.create("T", [{"id": "a", "role": "worker"}])
        loaded = TeamDefinition.load(team.id)
        assert loaded.id == team.id
        assert loaded.name == "T"
        assert loaded.members == [{"id": "a", "role": "worker"}]

    def test_load_not_found(self, teams_dir):
        with pytest.raises(FileNotFoundError):
            TeamDefinition.load("nonexistent")

    def test_list_all(self, teams_dir):
        TeamDefinition.create("T1", [{"id": "a", "role": "worker"}])
        TeamDefinition.create("T2", [{"id": "b", "role": "worker"}])
        teams = TeamDefinition.list_all()
        assert len(teams) == 2

    def test_save_updates(self, teams_dir):
        team = TeamDefinition.create("T", [{"id": "a", "role": "worker"}])
        team.status = "running"
        team.save()
        reloaded = TeamDefinition.load(team.id)
        assert reloaded.status == "running"

    def test_load_ignores_legacy_fields(self, teams_dir):
        """Loading a team.json with legacy fields works."""
        import json

        team_dir = teams_dir / "legacy"
        team_dir.mkdir()
        (team_dir / "team.json").write_text(json.dumps({
            "id": "legacy",
            "name": "Legacy Team",
            "members": [{"id": "alice", "role": "leader"}],
            "leader": "alice",
            "status": "created",
            "created_at": "2026-01-01",
        }))
        loaded = TeamDefinition.load("legacy")
        assert loaded.name == "Legacy Team"
        assert loaded.members == [{"id": "alice", "role": "leader"}]
