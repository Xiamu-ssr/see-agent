"""Unit tests for TeamManager."""

from __future__ import annotations

from unittest.mock import patch

import pytest

from see_agent.team.definition import TeamDefinition  # noqa: I001
from see_agent.team.manager import TeamManager

FAKE_CONFIG = {
    "llm": {
        "base_url": "http://localhost:1234/v1",
        "api_key": "fake",
        "model": "fake-model",
    },
    "max_steps": 2,
    "memory": {"enabled": False},
}


@pytest.fixture
def teams_dir(tmp_path):
    d = tmp_path / "teams"
    d.mkdir()
    with patch("see_agent.team.definition.TEAMS_DIR", d), \
         patch("see_agent.team.manager.TEAMS_DIR", d):
        yield d


@pytest.fixture
def agents_dir(tmp_path):
    d = tmp_path / "agents"
    d.mkdir()
    with patch("see_agent.agent.definition.AGENTS_DIR", d), \
         patch("see_agent.config.AGENTS_DIR", d):
        yield d


class TestTeamManager:

    def test_init(self, teams_dir):
        team_def = TeamDefinition.create("T", ["a", "b"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        assert mgr._bus is not None
        assert mgr._board is not None

    def test_register_team_tools(self, teams_dir):
        from see_agent.hand.tool import ToolRegistry

        team_def = TeamDefinition.create("T", ["a", "b"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        registry = ToolRegistry()
        mgr._register_team_tools(registry, "a")
        tool_names = list(registry._tools.keys())
        expected = [
            "send_message", "list_tasks", "create_task",
            "claim_task", "complete_task", "update_task", "assign_task",
        ]
        for name in expected:
            assert name in tool_names

    def test_build_team_context_leader(self, teams_dir, agents_dir):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("a", name="Alice", role="leader")
        AgentDefinition.create("b", name="Bob", role="coder")
        team_def = TeamDefinition.create("T", ["a", "b"], leader="a")
        mgr = TeamManager(team_def, FAKE_CONFIG)
        ctx = mgr._build_team_context("a")
        assert "Team Leader" in ctx
        assert "Team" in ctx

    def test_build_team_context_worker(self, teams_dir, agents_dir):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("a", name="Alice", role="leader")
        AgentDefinition.create("b", name="Bob", role="coder")
        team_def = TeamDefinition.create("T", ["a", "b"], leader="a")
        mgr = TeamManager(team_def, FAKE_CONFIG)
        ctx = mgr._build_team_context("b")
        assert "Team Worker" in ctx

    def test_build_agent_task(self, teams_dir, agents_dir):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("a", name="Alice", role="leader")
        team_def = TeamDefinition.create("T", ["a"], leader="a")
        mgr = TeamManager(team_def, FAKE_CONFIG)
        task_str = mgr._build_agent_task("a", "Fix the bug")
        assert "Fix the bug" in task_str
        assert "Team Context" in task_str

    @pytest.mark.asyncio
    async def test_stop(self, teams_dir):
        team_def = TeamDefinition.create("T", ["a"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        await mgr.stop()
        assert mgr._stopped is True
        reloaded = TeamDefinition.load(team_def.id)
        assert reloaded.status == "stopped"
