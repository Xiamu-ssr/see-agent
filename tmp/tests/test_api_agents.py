"""API tests for agent management endpoints."""

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
        "agent": {"max_steps": 5},
    }
    from see_agent.server.supervisor import AgentSupervisor
    app.state.supervisor = AgentSupervisor({})
    from see_agent.server.message_router import MessageRouter
    app.state.message_router = MessageRouter(app.state.supervisor)
    with TestClient(app, raise_server_exceptions=True) as c:
        yield c


class TestAgentAPI:

    def test_update_agent(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("alice")
        resp = client.put(
            "/api/agents/alice",
            json={"agent": {"max_steps": 200}},
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["id"] == "alice"
        reloaded = AgentDefinition.load("alice")
        assert reloaded.agent["max_steps"] == 200

    def test_update_agent_not_found(self, client):
        resp = client.put(
            "/api/agents/ghost",
            json={"agent": {"max_steps": 1}},
        )
        assert resp.status_code == 404

    def test_update_agent_partial(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("bob")
        resp = client.put(
            "/api/agents/bob",
            json={"tools": {"denied": ["shell"]}},
        )
        assert resp.status_code == 200
        reloaded = AgentDefinition.load("bob")
        assert reloaded.tools == {"denied": ["shell"]}


class TestListAgents:

    def test_list_empty(self, client):
        resp = client.get("/api/agents")
        assert resp.status_code == 200
        assert resp.json() == []

    def test_list_global(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("a1")
        resp = client.get("/api/agents")
        assert resp.status_code == 200
        data = resp.json()
        assert len(data) == 1
        assert data[0]["id"] == "a1"
        assert data[0]["team_id"] is None
        assert data[0]["status"] == "idle"

    def test_list_with_team(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition
        from see_agent.team.definition import TeamDefinition

        # Create agent in global AGENTS_DIR, then create team referencing it.
        AgentDefinition.create("t1")
        TeamDefinition.create("MyTeam", [{"id": "t1", "role": "leader"}], leader="t1")

        resp = client.get("/api/agents")
        data = resp.json()
        team_entries = [e for e in data if e["team_id"] is not None]
        assert len(team_entries) == 1
        assert team_entries[0]["id"] == "t1"
        assert team_entries[0]["team_name"] == "MyTeam"


class TestGetAgent:

    def test_get_detail(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("alice")
        resp = client.get("/api/agents/alice")
        assert resp.status_code == 200
        data = resp.json()
        assert data["id"] == "alice"
        assert data["has_soul"] is True
        assert data["team_id"] is None

    def test_get_not_found(self, client):
        resp = client.get("/api/agents/ghost")
        assert resp.status_code == 404

    def test_get_with_soul(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("soul_agent")
        agents_dir = workspace / "agents"
        (agents_dir / "soul_agent" / "SOUL.md").write_text("I have a soul")
        resp = client.get("/api/agents/soul_agent")
        assert resp.status_code == 200
        assert resp.json()["has_soul"] is True


class TestCreateAgent:

    def test_create(self, client, workspace):
        resp = client.post(
            "/api/agents",
            json={"id": "new1"},
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["id"] == "new1"
        # Verify on disk
        agents_dir = workspace / "agents"
        assert (agents_dir / "new1" / "agent.json").exists()

    def test_create_with_config(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        resp = client.post(
            "/api/agents",
            json={
                "id": "cfg1",
                "agent": {"max_steps": 100},
                "tools": {"denied": ["shell"]},
            },
        )
        assert resp.status_code == 200
        loaded = AgentDefinition.load("cfg1")
        assert loaded.agent == {"max_steps": 100}
        assert loaded.tools == {"denied": ["shell"]}

    def test_create_duplicate(self, client, workspace):
        client.post(
            "/api/agents",
            json={"id": "dup"},
        )
        resp = client.post(
            "/api/agents",
            json={"id": "dup"},
        )
        assert resp.status_code == 409
