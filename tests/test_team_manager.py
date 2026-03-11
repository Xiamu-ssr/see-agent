"""Unit tests for TeamManager."""

from __future__ import annotations

from pathlib import Path
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
         patch("see_agent.team.manager.TEAMS_DIR", d), \
         patch("see_agent.ipc.router.TEAMS_DIR", d):
        yield d


@pytest.fixture
def run_dir(tmp_path):
    d = tmp_path / "run"
    d.mkdir()
    with patch("see_agent.ipc.router.RUN_DIR", d), \
         patch("see_agent.config.RUN_DIR", d):
        yield d


@pytest.fixture
def agents_dir(tmp_path):
    d = tmp_path / "agents"
    d.mkdir()
    tmpl = Path(__file__).resolve().parent.parent / "see_agent" / "templates"
    with patch("see_agent.agent.definition.AGENTS_DIR", d), \
         patch("see_agent.config.AGENTS_DIR", d), \
         patch("see_agent.agent.definition._TEMPLATE_DIR", tmpl):
        yield d


class TestTeamManager:

    def test_init(self, teams_dir, run_dir):
        team_def = TeamDefinition.create("T", ["a", "b"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        assert mgr._router is None
        assert mgr._processes == {}

    def test_build_team_context_leader(self, teams_dir, run_dir, agents_dir):
        from see_agent.agent.definition import AgentDefinition
        from see_agent.ipc.router import AgentRouter

        AgentDefinition.create("a", name="Alice", role="leader")
        AgentDefinition.create("b", name="Bob", role="coder")
        team_def = TeamDefinition.create("T", ["a", "b"], leader="a")
        mgr = TeamManager(team_def, FAKE_CONFIG)

        # Set up a router so _build_team_context can access the board.
        mgr._router = AgentRouter(team_def.id)
        ctx = mgr._build_team_context("a")
        assert "Team Leader" in ctx
        assert "Team" in ctx

    def test_build_team_context_worker(self, teams_dir, run_dir, agents_dir):
        from see_agent.agent.definition import AgentDefinition
        from see_agent.ipc.router import AgentRouter

        AgentDefinition.create("a", name="Alice", role="leader")
        AgentDefinition.create("b", name="Bob", role="coder")
        team_def = TeamDefinition.create("T", ["a", "b"], leader="a")
        mgr = TeamManager(team_def, FAKE_CONFIG)
        mgr._router = AgentRouter(team_def.id)
        ctx = mgr._build_team_context("b")
        assert "Team Worker" in ctx

    @pytest.mark.asyncio
    async def test_stop(self, teams_dir, run_dir):
        team_def = TeamDefinition.create("T", ["a"])
        mgr = TeamManager(team_def, FAKE_CONFIG)
        await mgr.stop()
        assert mgr._stopped is True
        reloaded = TeamDefinition.load(team_def.id)
        assert reloaded.status == "stopped"
