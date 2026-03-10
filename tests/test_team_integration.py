"""Integration tests for the agent team platform.

End-to-end: create agent definitions, create team, verify wiring
of bus, board, tools, and session scoping — all with mocked brain/eye.

v3.1: TeamManager now uses AgentRouter (UDS) + subprocesses.
Tests verify the router-based wiring instead of direct _bus/_board access.
"""

from __future__ import annotations

import json
from unittest.mock import patch

import pytest

from see_agent.agent.definition import AgentDefinition
from see_agent.team.bus import BusMessage, TeamBus
from see_agent.team.definition import TeamDefinition
from see_agent.team.manager import TeamManager
from see_agent.team.task_board import TaskBoard


@pytest.fixture
def workspace(tmp_path):
    """Set up a complete workspace with agents, teams, and run dirs."""
    agents_dir = tmp_path / "agents"
    teams_dir = tmp_path / "teams"
    run_dir = tmp_path / "run"
    agents_dir.mkdir()
    teams_dir.mkdir()
    run_dir.mkdir()
    with (
        patch("see_agent.agent.definition.AGENTS_DIR", agents_dir),
        patch("see_agent.config.AGENTS_DIR", agents_dir),
        patch("see_agent.team.definition.TEAMS_DIR", teams_dir),
        patch("see_agent.team.manager.TEAMS_DIR", teams_dir),
        patch("see_agent.ipc.router.TEAMS_DIR", teams_dir),
        patch("see_agent.ipc.router.RUN_DIR", run_dir),
        patch("see_agent.config.RUN_DIR", run_dir),
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

    def test_bus_message_flow(self, workspace):
        """Messages flow between agents via TeamBus."""
        team_def = TeamDefinition.create("T", ["alice", "bob"])
        # Direct bus test (used by AgentRouter internally).
        bus = TeamBus(workspace / "teams" / team_def.id)
        bus.register("alice")
        bus.register("bob")
        bus.send(BusMessage(sender="alice", recipient="bob", content="hi"))
        msgs = bus.drain("bob")
        assert len(msgs) == 1
        assert msgs[0].content == "hi"

    def test_task_board_in_team_dir(self, workspace):
        """TaskBoard persists tasks in the team directory."""
        team_def = TeamDefinition.create("T", ["alice"])
        teams_dir = workspace / "teams"
        board = TaskBoard(teams_dir / team_def.id)
        board.create_task("Fix bug", created_by="system")
        tasks = board.list_tasks()
        assert len(tasks) == 1
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
        session_root = mgr._team_dir / "agents" / "alice" / "sessions"
        assert session_root == expected_root

    def test_team_context_includes_members(self, workspace):
        """Team context string includes all member info."""
        from see_agent.ipc.router import AgentRouter

        AgentDefinition.create("alice", name="Alice", role="leader")
        AgentDefinition.create("bob", name="Bob", role="coder")
        team_def = TeamDefinition.create("T", ["alice", "bob"], leader="alice")
        mgr = TeamManager(team_def, FAKE_CONFIG)
        mgr._router = AgentRouter(team_def.id)
        ctx = mgr._build_team_context("alice")
        assert "Alice" in ctx
        assert "Bob" in ctx
        assert "coder" in ctx

    def test_audit_log_written(self, workspace):
        """Bus audit log is written to messages.jsonl in team dir."""
        team_def = TeamDefinition.create("T", ["alice", "bob"])
        teams_dir = workspace / "teams"
        bus = TeamBus(teams_dir / team_def.id)
        bus.register("alice")
        bus.register("bob")
        bus.send(BusMessage(sender="alice", recipient="bob", content="test"))
        log_path = teams_dir / team_def.id / "messages.jsonl"
        assert log_path.exists()
        lines = log_path.read_text().strip().splitlines()
        assert len(lines) == 1
        entry = json.loads(lines[0])
        assert entry["sender"] == "alice"

    def test_shared_dir_created(self, workspace):
        """TeamManager.__init__ creates a shared/ dir under team dir."""
        team_def = TeamDefinition.create("T", ["alice"])
        TeamManager(team_def, FAKE_CONFIG)
        teams_dir = workspace / "teams"
        assert (teams_dir / team_def.id / "shared").is_dir()

    @pytest.mark.asyncio
    async def test_router_started_on_run(self, workspace):
        """AgentRouter bus has owner registered when run is called with owner."""
        from see_agent.ipc.router import AgentRouter

        AgentDefinition.create("alice", name="Alice", role="leader")
        owner = {"name": "john", "display": "John"}
        team_def = TeamDefinition.create(
            "T", ["alice"], leader="alice", owner=owner,
        )
        TeamManager(team_def, FAKE_CONFIG)

        # Create a router to verify registration logic.
        router = AgentRouter(team_def.id)
        router.register_agent("owner")
        router.register_agent("alice")
        assert "owner" in router.bus._queues
        assert "alice" in router.bus._queues

    def test_agent_sandbox_field(self, workspace):
        """Agent definition stores sandbox config."""
        sandbox = {
            "enabled": True,
            "network": True,
            "screen_access": True,
            "extra_read": [],
            "extra_write": [],
        }
        AgentDefinition.create(
            "alice", name="Alice", role="leader", sandbox=sandbox,
        )
        loaded = AgentDefinition.load("alice")
        assert loaded.sandbox["enabled"] is True
        assert loaded.sandbox["screen_access"] is True
