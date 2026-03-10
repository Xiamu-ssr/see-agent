"""API tests for agent management endpoints."""

from __future__ import annotations

from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def workspace(tmp_path):
    agents_dir = tmp_path / "agents"
    agents_dir.mkdir()
    teams_dir = tmp_path / "teams"
    teams_dir.mkdir()
    with (
        patch("see_agent.agent.definition.AGENTS_DIR", agents_dir),
        patch("see_agent.agent.definition.TEAMS_DIR", teams_dir),
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
    app.state.team_managers = {}
    with TestClient(app, raise_server_exceptions=True) as c:
        yield c


class TestAgentAPI:

    def test_update_agent(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("alice", name="Alice", role="coder")
        resp = client.put(
            "/api/agents/alice",
            json={"name": "Alice Updated", "role": "leader"},
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["name"] == "Alice Updated"
        assert data["role"] == "leader"
        reloaded = AgentDefinition.load("alice")
        assert reloaded.name == "Alice Updated"

    def test_update_agent_not_found(self, client):
        resp = client.put(
            "/api/agents/ghost",
            json={"name": "Ghost"},
        )
        assert resp.status_code == 404

    def test_update_agent_partial(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("bob", name="Bob", role="coder")
        resp = client.put(
            "/api/agents/bob",
            json={"tools_config": {"denied": ["shell"]}},
        )
        assert resp.status_code == 200
        reloaded = AgentDefinition.load("bob")
        assert reloaded.tools_config == {"denied": ["shell"]}
        assert reloaded.name == "Bob"


class TestListAgents:

    def test_list_empty(self, client):
        resp = client.get("/api/agents/")
        assert resp.status_code == 200
        assert resp.json() == []

    def test_list_global(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("a1", name="Agent1", role="coder")
        resp = client.get("/api/agents/")
        assert resp.status_code == 200
        data = resp.json()
        assert len(data) == 1
        assert data[0]["id"] == "a1"
        assert data[0]["team_id"] is None
        assert data[0]["status"] == "idle"

    def test_list_with_team(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition
        from see_agent.team.definition import TeamDefinition

        TeamDefinition.create("MyTeam", ["t1"], leader="t1")
        teams_dir = workspace / "teams"
        # find the team dir (hex id)
        team_dirs = [d for d in teams_dir.iterdir() if d.is_dir()]
        assert len(team_dirs) == 1
        team_dir = team_dirs[0]
        team_agents = team_dir / "agents"
        team_agents.mkdir()
        AgentDefinition(id="t1", name="TeamAgent").save_to(team_agents)

        resp = client.get("/api/agents/")
        data = resp.json()
        team_entries = [e for e in data if e["team_id"] is not None]
        assert len(team_entries) == 1
        assert team_entries[0]["id"] == "t1"
        assert team_entries[0]["team_name"] == "MyTeam"


class TestGetAgent:

    def test_get_detail(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("alice", name="Alice", role="coder")
        resp = client.get("/api/agents/alice")
        assert resp.status_code == 200
        data = resp.json()
        assert data["id"] == "alice"
        assert data["name"] == "Alice"
        assert data["has_soul"] is False
        assert data["team_id"] is None

    def test_get_not_found(self, client):
        resp = client.get("/api/agents/ghost")
        assert resp.status_code == 404

    def test_get_with_soul(self, client, workspace):
        from see_agent.agent.definition import AgentDefinition

        AgentDefinition.create("soul_agent", name="SoulAgent")
        agents_dir = workspace / "agents"
        (agents_dir / "soul_agent" / "SOUL.md").write_text("I have a soul")
        resp = client.get("/api/agents/soul_agent")
        assert resp.status_code == 200
        assert resp.json()["has_soul"] is True


class TestCreateAgent:

    def test_create(self, client, workspace):
        resp = client.post(
            "/api/agents/",
            json={"id": "new1", "name": "New Agent", "role": "tester"},
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["id"] == "new1"
        assert data["name"] == "New Agent"
        # Verify on disk
        agents_dir = workspace / "agents"
        assert (agents_dir / "new1" / "agent.json").exists()

    def test_create_duplicate(self, client, workspace):
        client.post(
            "/api/agents/",
            json={"id": "dup", "name": "Dup"},
        )
        resp = client.post(
            "/api/agents/",
            json={"id": "dup", "name": "Dup Again"},
        )
        assert resp.status_code == 409
