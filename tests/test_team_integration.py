"""Integration tests for the agent team platform.

v3.5: Tests verify AgentDefinition, TeamDefinition, TaskBoard,
and agent workspace setup — without TeamManager or TeamBus.
"""

from __future__ import annotations

import json
from pathlib import Path as _Path
from unittest.mock import patch

import pytest

from see_agent.agent.definition import AgentDefinition
from see_agent.team.definition import TeamDefinition
from see_agent.team.task_board import TaskBoard

_REAL_TEMPLATE_DIR = (
    _Path(__file__).resolve().parent.parent / "see_agent" / "templates"
)


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
        patch("see_agent.agent.definition._TEMPLATE_DIR", _REAL_TEMPLATE_DIR),
        patch("see_agent.config.AGENTS_DIR", agents_dir),
        patch("see_agent.team.definition.TEAMS_DIR", teams_dir),
        patch("see_agent.config.RUN_DIR", run_dir),
    ):
        yield tmp_path


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
        team = TeamDefinition.create(
            "Alpha", ["alice", "bob"], leader="alice",
        )
        loaded = TeamDefinition.load(team.id)
        assert loaded.members == ["alice", "bob"]
        assert loaded.leader == "alice"

    def test_agent_get_team(self, workspace):
        """Agent.get_team() returns team id from team.json."""
        AgentDefinition.create("alice", name="Alice", role="leader")
        AgentDefinition.create("bob", name="Bob", role="coder")
        TeamDefinition.create("Alpha", ["alice"], leader="alice")
        alice = AgentDefinition.load("alice")
        bob = AgentDefinition.load("bob")
        assert alice.get_team() is not None
        assert bob.get_team() is None

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

    def test_agent_workspace_template(self, workspace):
        """Agent creation sets up workspace with template files."""
        AgentDefinition.create("alice", name="Alice", role="leader")
        agents_dir = workspace / "agents"
        ws = agents_dir / "alice" / "workspace"
        assert ws.is_dir()
        assert (ws / "AGENTS.md").exists()
        assert (ws / "SOUL.md").exists()

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
