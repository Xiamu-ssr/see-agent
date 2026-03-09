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
        team = TeamDefinition.create("My Team", ["alice", "bob"], leader="alice")
        assert team.name == "My Team"
        assert team.members == ["alice", "bob"]
        assert team.leader == "alice"
        assert team.status == "created"
        assert (teams_dir / team.id / "team.json").exists()

    def test_load(self, teams_dir):
        team = TeamDefinition.create("T", ["a"])
        loaded = TeamDefinition.load(team.id)
        assert loaded.id == team.id
        assert loaded.name == "T"
        assert loaded.members == ["a"]

    def test_load_not_found(self, teams_dir):
        with pytest.raises(FileNotFoundError):
            TeamDefinition.load("nonexistent")

    def test_list_all(self, teams_dir):
        TeamDefinition.create("T1", ["a"])
        TeamDefinition.create("T2", ["b"])
        teams = TeamDefinition.list_all()
        assert len(teams) == 2

    def test_save_updates(self, teams_dir):
        team = TeamDefinition.create("T", ["a"])
        team.status = "running"
        team.save()
        reloaded = TeamDefinition.load(team.id)
        assert reloaded.status == "running"

    def test_owner_round_trip(self, teams_dir):
        owner = {"name": "john", "display": "John Doe"}
        team = TeamDefinition.create("T", ["a"], owner=owner)
        loaded = TeamDefinition.load(team.id)
        assert loaded.owner == owner
        assert loaded.owner["display"] == "John Doe"
