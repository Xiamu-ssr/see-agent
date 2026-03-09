"""API tests for agent management endpoints."""

from __future__ import annotations

from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def workspace(tmp_path):
    agents_dir = tmp_path / "agents"
    agents_dir.mkdir()
    with (
        patch("see_agent.agent.definition.AGENTS_DIR", agents_dir),
        patch("see_agent.config.AGENTS_DIR", agents_dir),
    ):
        yield agents_dir


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
        # Verify persisted.
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
        assert reloaded.name == "Bob"  # unchanged
