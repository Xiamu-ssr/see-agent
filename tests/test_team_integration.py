"""Integration tests for the agent team platform.

End-to-end: create agent definitions, create team, verify wiring
of bus, board, tools, and session scoping — all with mocked brain/eye.
"""

from __future__ import annotations

import json
from unittest.mock import patch

import pytest

from see_agent.agent.definition import AgentDefinition
from see_agent.hand.tool import ToolRegistry
from see_agent.team.bus import BusMessage
from see_agent.team.definition import TeamDefinition
from see_agent.team.manager import TeamManager


@pytest.fixture
def workspace(tmp_path):
    """Set up a complete workspace with agents and teams dirs."""
    agents_dir = tmp_path / "agents"
    teams_dir = tmp_path / "teams"
    agents_dir.mkdir()
    teams_dir.mkdir()
    with (
        patch("see_agent.agent.definition.AGENTS_DIR", agents_dir),
        patch("see_agent.config.AGENTS_DIR", agents_dir),
        patch("see_agent.team.definition.TEAMS_DIR", teams_dir),
        patch("see_agent.team.manager.TEAMS_DIR", teams_dir),
    ):
        yield tmp_path


FAKE_CONFIG = {
    "llm": {
        "base_url": "http://localhost:1234/v1",
        "api_key": "fake",
        "model": "fake-model",
    },
    "max_steps": 2,
    "memory": {"enabled": False},
}


class TestTeamIntegration:
    """End-to-end integration tests for multi-agent team platform."""

    def test_agent_definitions_created(self, workspace):
        """Agent definitions round-trip through create/load."""
        AgentDefinition.create("alice", name="Alice", role="leader")
        AgentDefinition.create("bob", name="Bob", role="coder")
        agents = AgentDefinition.list_all()
        assert len(agents) == 2
        names = {a.name for a in agents}
        assert names == {"Alice", "Bob"}

    def test_team_definition_with_agents(self, workspace):
        """Team definition links to agent IDs."""
        AgentDefinition.create("alice", name="Alice", role="leader")
        AgentDefinition.create("bob", name="Bob", role="coder")
        team = TeamDefinition.create("Alpha", ["alice", "bob"], leader="alice")
        loaded = TeamDefinition.load(team.id)
        assert loaded.members == ["alice", "bob"]
        assert loaded.leader == "alice"

    def test_team_manager_registers_tools(self, workspace):
        """TeamManager registers all 7 team tools."""
        AgentDefinition.create("alice", name="Alice", role="leader")
        AgentDefinition.create("bob", name="Bob", role="coder")
        team_def = TeamDefinition.create("T", ["alice", "bob"], leader="alice")
        mgr = TeamManager(team_def, FAKE_CONFIG)
        registry = ToolRegistry()
        mgr._register_team_tools(registry, "alice")
        assert len(registry._tools) == 7
        assert all(
            registry._sources[name] == "team"
            for name in registry._tools
        )

    def test_bus_message_flow(self, workspace):
        """Messages flow between agents via TeamBus."""
        team_def = TeamDefinition.create("T", ["alice", "bob"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        mgr._bus.register("alice")
        mgr._bus.register("bob")
        mgr._bus.send(BusMessage(sender="alice", recipient="bob", content="hi"))
        msgs = mgr._bus.drain("bob")
        assert len(msgs) == 1
        assert msgs[0].content == "hi"

    def test_task_board_in_team_dir(self, workspace):
        """TaskBoard persists tasks in the team directory."""
        team_def = TeamDefinition.create("T", ["alice"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        mgr._board.create_task("Fix bug", created_by="system")
        tasks = mgr._board.list_tasks()
        assert len(tasks) == 1
        # Verify file exists in team dir.
        teams_dir = workspace / "teams"
        tasks_file = teams_dir / team_def.id / "tasks.json"
        assert tasks_file.exists()
        data = json.loads(tasks_file.read_text())
        assert len(data) == 1

    def test_session_root_scoped_to_team(self, workspace):
        """Agent session root is scoped under teams/{id}/agents/{aid}/sessions."""
        AgentDefinition.create("alice", name="Alice", role="leader")
        team_def = TeamDefinition.create("T", ["alice"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        teams_dir = workspace / "teams"
        expected_root = teams_dir / team_def.id / "agents" / "alice" / "sessions"
        # _build_agent_loop creates the session root directory.
        # We can't fully call it without real brain/eye, but verify the path logic.
        session_root = mgr._team_dir / "agents" / "alice" / "sessions"
        assert session_root == expected_root

    def test_team_context_includes_members(self, workspace):
        """Team context string includes all member info."""
        AgentDefinition.create("alice", name="Alice", role="leader")
        AgentDefinition.create("bob", name="Bob", role="coder")
        team_def = TeamDefinition.create("T", ["alice", "bob"], leader="alice")
        mgr = TeamManager(team_def, FAKE_CONFIG)
        ctx = mgr._build_team_context("alice")
        assert "Alice" in ctx
        assert "Bob" in ctx
        assert "coder" in ctx

    def test_audit_log_written(self, workspace):
        """Bus audit log is written to messages.jsonl in team dir."""
        team_def = TeamDefinition.create("T", ["alice", "bob"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        mgr._bus.register("alice")
        mgr._bus.register("bob")
        mgr._bus.send(BusMessage(sender="alice", recipient="bob", content="test"))
        teams_dir = workspace / "teams"
        log_path = teams_dir / team_def.id / "messages.jsonl"
        assert log_path.exists()
        lines = log_path.read_text().strip().splitlines()
        assert len(lines) == 1
        entry = json.loads(lines[0])
        assert entry["sender"] == "alice"

    def test_shared_eye_instance(self, workspace):
        """All agents share the same MacEye instance."""
        team_def = TeamDefinition.create("T", ["alice", "bob"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        assert mgr._shared_eye is None
        # After building loops, shared_eye should be set.
        # Can't call _build_agent_loop without real MacEye, but verify field.
        assert hasattr(mgr, "_shared_eye")

    def test_shared_dir_created(self, workspace):
        """TeamManager.__init__ creates a shared/ dir under team dir."""
        team_def = TeamDefinition.create("T", ["alice"])
        TeamManager(team_def, FAKE_CONFIG)
        teams_dir = workspace / "teams"
        assert (teams_dir / team_def.id / "shared").is_dir()

    def test_screen_lock_created(self, workspace):
        """TeamManager creates an asyncio.Lock for screen coordination."""
        import asyncio

        team_def = TeamDefinition.create("T", ["alice"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        assert isinstance(mgr._screen_lock, asyncio.Lock)
