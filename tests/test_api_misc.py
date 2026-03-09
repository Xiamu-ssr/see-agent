"""API tests for misc endpoints (tools, skills, config)."""

from __future__ import annotations

from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def workspace(tmp_path):
    with (
        patch("see_agent.config.CONFIG_PATH", tmp_path / "config.json"),
        patch("see_agent.config.WORKSPACE_DIR", tmp_path),
        patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
        patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
        patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
        patch("see_agent.config.AGENTS_DIR", tmp_path / "agents"),
        patch("see_agent.config.TEAMS_DIR", tmp_path / "teams"),
    ):
        for d in ("sessions", "logs", "skills", "agents", "teams"):
            (tmp_path / d).mkdir(exist_ok=True)
        yield tmp_path


@pytest.fixture()
def client(workspace):
    import json

    from see_agent.server.app import app

    # Write config.json so lifespan picks it up.
    config = {
        "llm": {
            "base_url": "http://test/v1",
            "api_key": "sk-testkey1234567890",
            "model": "m",
        },
        "max_steps": 5,
        "skills_dirs": [],
    }
    (workspace / "config.json").write_text(json.dumps(config))
    with TestClient(app, raise_server_exceptions=True) as c:
        yield c


class TestToolsAPI:

    def test_list_tools(self, client):
        resp = client.get("/api/tools")
        assert resp.status_code == 200
        tools = resp.json()
        assert isinstance(tools, list)
        names = {t["name"] for t in tools}
        assert "click" in names
        assert "finished" in names


class TestSkillsAPI:

    def test_list_skills_empty(self, client):
        resp = client.get("/api/skills")
        assert resp.status_code == 200
        assert resp.json() == []


class TestConfigAPI:

    def test_get_config_masks_key(self, client):
        resp = client.get("/api/config")
        assert resp.status_code == 200
        data = resp.json()
        key = data["llm"]["api_key"]
        assert "****" in key
        # Original key should NOT appear.
        assert "testkey" not in key

    def test_update_config(self, client, workspace):
        resp = client.put(
            "/api/config",
            json={"config": {"max_steps": 99}},
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "updated"
        # Verify updated in app state.
        resp2 = client.get("/api/config")
        assert resp2.json()["max_steps"] == 99
