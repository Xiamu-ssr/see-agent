"""Unit tests for AgentDefinition."""

import json
from unittest.mock import patch

import pytest

from see_agent.agent.definition import AgentDefinition


@pytest.fixture
def agents_dir(tmp_path):
    d = tmp_path / "agents"
    d.mkdir()
    teams = tmp_path / "teams"
    teams.mkdir()
    with (
        patch("see_agent.agent.definition.AGENTS_DIR", d),
        patch("see_agent.agent.definition.TEAMS_DIR", teams),
    ):
        yield d


@pytest.fixture
def teams_dir(tmp_path):
    """Return the teams dir (sibling of agents_dir)."""
    return tmp_path / "teams"


class TestAgentDefinition:
    """Tests for AgentDefinition CRUD."""

    def test_create_and_load(self, agents_dir):
        AgentDefinition.create("alice", name="Alice", role="researcher")
        assert (agents_dir / "alice" / "agent.json").exists()

        loaded = AgentDefinition.load("alice")
        assert loaded.id == "alice"
        assert loaded.name == "Alice"
        assert loaded.role == "researcher"

    def test_save_overwrites(self, agents_dir):
        defn = AgentDefinition.create("bob", name="Bob")
        defn.role = "coder"
        defn.save()

        loaded = AgentDefinition.load("bob")
        assert loaded.role == "coder"

    def test_list_all(self, agents_dir):
        AgentDefinition.create("a1", name="Agent 1")
        AgentDefinition.create("a2", name="Agent 2")

        agents = AgentDefinition.list_all()
        assert len(agents) == 2
        names = {a.name for a in agents}
        assert names == {"Agent 1", "Agent 2"}

    def test_list_all_empty(self, agents_dir):
        assert AgentDefinition.list_all() == []

    def test_load_nonexistent(self, agents_dir):
        with pytest.raises(FileNotFoundError, match="Agent not found"):
            AgentDefinition.load("ghost")

    def test_config_overrides_saved(self, agents_dir):
        AgentDefinition.create(
            "custom", name="Custom",
            config_overrides={"max_steps": 100},
        )
        data = json.loads(
            (agents_dir / "custom" / "agent.json").read_text()
        )
        assert data["config_overrides"]["max_steps"] == 100

    def test_soul_path_round_trip(self, agents_dir):
        from pathlib import Path

        AgentDefinition.create(
            "soul", name="Soul",
            soul_path=Path("/tmp/test_soul.md"),
        )
        loaded = AgentDefinition.load("soul")
        assert loaded.soul_path == Path("/tmp/test_soul.md")

    def test_tools_config_saved(self, agents_dir):
        AgentDefinition.create(
            "filtered", name="Filtered",
            tools_config={"allowed": ["click", "shell"]},
        )
        loaded = AgentDefinition.load("filtered")
        assert loaded.tools_config["allowed"] == ["click", "shell"]

    def test_get_merged_config(self, agents_dir, tmp_path):
        AgentDefinition.create(
            "merge", name="Merge",
            config_overrides={"max_steps": 200},
        )
        config_path = tmp_path / "config.json"
        config_path.write_text(json.dumps({
            "llm": {"api_key": "k", "model": "base"},
            "max_steps": 50,
        }))
        with (
            patch("see_agent.config.CONFIG_PATH", config_path),
            patch("see_agent.config.WORKSPACE_DIR", tmp_path),
            patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
            patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
            patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
            patch("see_agent.config.AGENTS_DIR", agents_dir),
            patch("see_agent.config.TEAMS_DIR", tmp_path / "teams"),
        ):
            merged = AgentDefinition.load("merge").get_merged_config()
        assert merged["max_steps"] == 200
        assert merged["llm"]["model"] == "base"

    def test_list_all_skips_corrupted(self, agents_dir):
        """Corrupted agent.json is skipped gracefully."""
        bad_dir = agents_dir / "bad"
        bad_dir.mkdir()
        (bad_dir / "agent.json").write_text("not json")

        AgentDefinition.create("good", name="Good")
        agents = AgentDefinition.list_all()
        assert len(agents) == 1
        assert agents[0].id == "good"


class TestLoadFromSaveTo:
    """Tests for load_from / save_to."""

    def test_save_to_and_load_from(self, tmp_path):
        base = tmp_path / "custom"
        base.mkdir()
        defn = AgentDefinition(id="x", name="X", role="tester")
        defn.save_to(base)
        assert (base / "x" / "agent.json").exists()

        loaded = AgentDefinition.load_from(base, "x")
        assert loaded.id == "x"
        assert loaded.name == "X"

    def test_load_from_not_found(self, tmp_path):
        with pytest.raises(FileNotFoundError):
            AgentDefinition.load_from(tmp_path, "nope")


class TestListAllGlobal:
    """Tests for list_all_global."""

    def test_mixed(self, agents_dir, teams_dir):
        # Global agent
        AgentDefinition.create("g1", name="Global1")

        # Team-scoped agent
        team_agents = teams_dir / "team-abc" / "agents"
        team_agents.mkdir(parents=True)
        t_defn = AgentDefinition(id="t1", name="TeamAgent1")
        t_defn.save_to(team_agents)

        result = AgentDefinition.list_all_global()
        assert len(result) == 2
        ids = [(d.id, tid) for d, tid in result]
        assert ("g1", None) in ids
        assert ("t1", "team-abc") in ids

    def test_empty(self, agents_dir, teams_dir):
        result = AgentDefinition.list_all_global()
        assert result == []


class TestFind:
    """Tests for find."""

    def test_find_global(self, agents_dir, teams_dir):
        AgentDefinition.create("alice", name="Alice")
        defn, path, team_id = AgentDefinition.find("alice")
        assert defn.id == "alice"
        assert team_id is None
        assert path == agents_dir / "alice"

    def test_find_team(self, agents_dir, teams_dir):
        team_agents = teams_dir / "team-x" / "agents"
        team_agents.mkdir(parents=True)
        AgentDefinition(id="bob", name="Bob").save_to(team_agents)

        defn, path, team_id = AgentDefinition.find("bob")
        assert defn.id == "bob"
        assert team_id == "team-x"

    def test_find_not_found(self, agents_dir, teams_dir):
        with pytest.raises(FileNotFoundError, match="Agent not found"):
            AgentDefinition.find("ghost")
