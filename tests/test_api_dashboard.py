"""API tests for dashboard endpoint."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient

_REAL_TEMPLATE_DIR = (
    Path(__file__).resolve().parent.parent / "see_agent" / "templates"
)


@pytest.fixture()
def workspace(tmp_path):
    agents_dir = tmp_path / "agents"
    agents_dir.mkdir()
    teams_dir = tmp_path / "teams"
    teams_dir.mkdir()
    with (
        patch("see_agent.agent.definition.AGENTS_DIR", agents_dir),
        patch("see_agent.agent.definition._TEMPLATE_DIR", _REAL_TEMPLATE_DIR),
        patch("see_agent.config.AGENTS_DIR", agents_dir),
        patch("see_agent.config.TEAMS_DIR", teams_dir),
        patch("see_agent.team.definition.TEAMS_DIR", teams_dir),
    ):
        yield tmp_path


@pytest.fixture()
def client(workspace):
    from see_agent.server.app import app

    app.state.config = {
        "llm": {"base_url": "http://test/v1", "api_key": "k", "model": "m"},
        "max_steps": 5,
    }
    from see_agent.server.supervisor import AgentSupervisor
    app.state.supervisor = AgentSupervisor({})
    from see_agent.server.message_router import MessageRouter
    app.state.message_router = MessageRouter(app.state.supervisor)
    with TestClient(app, raise_server_exceptions=True) as c:
        yield c


class TestDashboard:

    def test_empty(self, client):
        resp = client.get("/api/dashboard")
        assert resp.status_code == 200
        data = resp.json()
        assert data["teams_count"] == 0
        assert data["agents_idle"] == 0
        assert data["total_tasks"] == 0

    def test_with_data(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition
        from see_agent.team.definition import TeamDefinition

        AgentDefinition.create("idle1", name="Idle1")
        TeamDefinition.create("T1", ["a1"])

        resp = client.get("/api/dashboard")
        assert resp.status_code == 200
        data = resp.json()
        assert data["teams_count"] == 1
        assert data["agents_idle"] == 1
