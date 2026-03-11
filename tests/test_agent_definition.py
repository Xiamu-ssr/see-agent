"""Unit tests for AgentDefinition."""

import json
from pathlib import Path
from unittest.mock import patch

import pytest

from see_agent.agent.definition import AgentDefinition

# Real template directory (so create() can copy AGENTS.md / SOUL.md).
_REAL_TEMPLATE_DIR = Path(__file__).resolve().parent.parent / "see_agent" / "templates"


@pytest.fixture
def agents_dir(tmp_path):
    d = tmp_path / "agents"
    d.mkdir()
    teams = tmp_path / "teams"
    teams.mkdir()
    with (
        patch("see_agent.agent.definition.AGENTS_DIR", d),
        patch("see_agent.agent.definition._TEMPLATE_DIR", _REAL_TEMPLATE_DIR),
        patch("see_agent.team.definition.TEAMS_DIR", teams),
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


class TestCreateSetup:
    """Tests for create() template and directory setup."""

    def test_create_generates_memory_dir(self, agents_dir):
        AgentDefinition.create("mem-agent", name="MemAgent")
        assert (agents_dir / "mem-agent" / "memory").is_dir()

    def test_create_generates_workspace_dir(self, agents_dir):
        AgentDefinition.create("ws-agent", name="WsAgent")
        ws_dir = agents_dir / "ws-agent" / "workspace"
        assert ws_dir.is_dir()
        assert (ws_dir / "AGENTS.md").exists()
        assert (ws_dir / "SOUL.md").exists()
        assert (ws_dir / "IDENTITY.md").exists()
        assert (ws_dir / "TOOLS.md").exists()
        assert (ws_dir / "USER.md").exists()

    def test_create_copies_agents_md(self, agents_dir):
        AgentDefinition.create("tmpl-agent", name="TmplAgent")
        agents_md = agents_dir / "tmpl-agent" / "AGENTS.md"
        assert agents_md.exists()
        content = agents_md.read_text(encoding="utf-8")
        assert "记忆管理" in content

    def test_create_copies_soul_md(self, agents_dir):
        AgentDefinition.create("soul-agent", name="SoulAgent")
        soul_md = agents_dir / "soul-agent" / "SOUL.md"
        assert soul_md.exists()

    def test_create_does_not_overwrite_existing(self, agents_dir):
        agent_dir = agents_dir / "custom-agent"
        agent_dir.mkdir(parents=True)
        (agent_dir / "AGENTS.md").write_text("custom content")
        AgentDefinition.create("custom-agent", name="Custom")
        assert (agent_dir / "AGENTS.md").read_text() == "custom content"

    def test_create_workspace_does_not_overwrite(self, agents_dir):
        agent_dir = agents_dir / "custom-ws"
        ws_dir = agent_dir / "workspace"
        ws_dir.mkdir(parents=True)
        (ws_dir / "AGENTS.md").write_text("my custom rules")
        AgentDefinition.create("custom-ws", name="CustomWS")
        assert (ws_dir / "AGENTS.md").read_text() == "my custom rules"


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


class TestFind:
    """Tests for find — only searches AGENTS_DIR."""

    def test_find_global(self, agents_dir, teams_dir):
        AgentDefinition.create("alice", name="Alice")
        defn, path = AgentDefinition.find("alice")
        assert defn.id == "alice"
        assert path == agents_dir / "alice"

    def test_find_not_found(self, agents_dir, teams_dir):
        with pytest.raises(FileNotFoundError, match="Agent not found"):
            AgentDefinition.find("ghost")


class TestGetTeam:
    """Tests for get_team()."""

    def test_agent_in_team(self, agents_dir, teams_dir):
        AgentDefinition.create("alice", name="Alice")

        team_dir = teams_dir / "team-x"
        team_dir.mkdir(parents=True)
        (team_dir / "team.json").write_text(json.dumps({
            "id": "team-x",
            "name": "X Team",
            "members": ["alice"],
        }))

        defn = AgentDefinition.load("alice")
        assert defn.get_team() == "team-x"

    def test_agent_not_in_team(self, agents_dir, teams_dir):
        AgentDefinition.create("bob", name="Bob")
        defn = AgentDefinition.load("bob")
        assert defn.get_team() is None
